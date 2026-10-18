//! Signing: the three headers Kalshi reads, made over `ring`.
//!
//! [`Credentials`] holds an API key id and the private key it signs with, an Ed25519 key or an
//! RSA key, read once from PEM. [`PredictionsBuilder::credentials`] installs them. A keyed client
//! signs every request but its ping, with a fresh timestamp and signature on every attempt, and
//! a client without credentials signs none.
//!
//! The text signed is the timestamp in milliseconds, the method and the path, joined with
//! nothing between them, and the path has no query. An Ed25519 key signs it as it is. An RSA key
//! signs with RSA-PSS over SHA-256, with MGF1 over SHA-256 and a salt as long as the digest. The
//! signature is base64, padded.

use std::fmt;
use std::path::Path;
use std::sync::Arc;

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use polyoxide_core::reqwest::header::{HeaderName, HeaderValue};
use polyoxide_core::{ApiError, Authenticator, DynAuthenticator, RequestParts};
use polyoxide_venue::{Class, Classify, Secret, UnixMillis};
use ring::rand::SystemRandom;
use ring::signature::{Ed25519KeyPair, RsaKeyPair, RSA_PSS_SHA256};

use crate::predictions::PredictionsBuilder;

/// The header that names the API key.
pub(crate) const ACCESS_KEY: &str = "KALSHI-ACCESS-KEY";
/// The header that carries the timestamp, in milliseconds.
pub(crate) const ACCESS_TIMESTAMP: &str = "KALSHI-ACCESS-TIMESTAMP";
/// The header that carries the base64 signature.
pub(crate) const ACCESS_SIGNATURE: &str = "KALSHI-ACCESS-SIGNATURE";

/// Why a key could not be read or used to sign.
///
/// Every variant is [`Class::InvalidRequest`]: the caller supplied a key Kalshi cannot use. No
/// message quotes the key's body, the base64 or a byte of it.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum CredentialsError {
    /// The text is not one armoured PEM block.
    #[error("the key is not valid PEM: {reason}")]
    Pem {
        /// What is wrong with the armour or the encoding.
        reason: String,
    },
    /// The PEM block is labelled as something other than a key Kalshi issues.
    #[error("a PEM labelled {label} is not a supported key; Kalshi keys are Ed25519 or RSA")]
    Label {
        /// The label on the BEGIN line.
        label: String,
    },
    /// The body is not a key `ring` can sign with.
    #[error("the PEM holds no Ed25519 or RSA key ring can sign with: {reason}")]
    Key {
        /// What `ring` rejected, in its own words.
        reason: String,
    },
    /// The key file could not be read.
    #[error("could not read the key file {path}: {reason}")]
    File {
        /// The path that was asked for.
        path: String,
        /// The operating system's reason.
        reason: String,
    },
    /// The key could not sign, or a header could not carry the result.
    #[error("signing failed: {reason}")]
    Sign {
        /// What failed.
        reason: String,
    },
}

/// A key that cannot be used is the caller's mistake, which no retry fixes.
impl Classify for CredentialsError {
    fn class(&self) -> Class {
        Class::InvalidRequest
    }
}

fn pem_error(reason: &str) -> CredentialsError {
    CredentialsError::Pem {
        reason: reason.to_owned(),
    }
}

/// An RSA key with the generator its signatures draw their salt from.
struct RsaSigner {
    pair: RsaKeyPair,
    rng: SystemRandom,
}

/// The private key, parsed. The RSA pair is boxed: it is the larger by far.
enum SigningKey {
    Ed25519(Ed25519KeyPair),
    Rsa(Box<RsaSigner>),
}

impl SigningKey {
    /// Reads a key from PEM.
    ///
    /// The label decides before the body is read: `RSA PRIVATE KEY` is PKCS#1 RSA, and
    /// `PRIVATE KEY` is PKCS#8, which cannot say what it holds, so it is tried as Ed25519 and
    /// then as RSA. Any other label, an encrypted key included, is refused.
    fn from_pem(text: &str) -> Result<Self, CredentialsError> {
        let (label, body) = unarmour(text)?;
        if label != "RSA PRIVATE KEY" && label != "PRIVATE KEY" {
            return Err(CredentialsError::Label {
                label: label.chars().take(64).collect(),
            });
        }
        let der = STANDARD
            .decode(body.as_bytes())
            .map_err(|_| pem_error("the body is not base64"))?;
        if label == "RSA PRIVATE KEY" {
            let pair = RsaKeyPair::from_der(&der).map_err(|rejected| CredentialsError::Key {
                reason: rejected.to_string(),
            })?;
            return Ok(Self::rsa(pair));
        }
        match Ed25519KeyPair::from_pkcs8_maybe_unchecked(&der) {
            Ok(pair) => Ok(Self::Ed25519(pair)),
            Err(not_ed25519) => match RsaKeyPair::from_pkcs8(&der) {
                Ok(pair) => Ok(Self::rsa(pair)),
                Err(not_rsa) => Err(CredentialsError::Key {
                    reason: format!("not Ed25519 ({not_ed25519}) and not RSA ({not_rsa})"),
                }),
            },
        }
    }

    fn rsa(pair: RsaKeyPair) -> Self {
        Self::Rsa(Box::new(RsaSigner {
            pair,
            rng: SystemRandom::new(),
        }))
    }

    fn sign(&self, message: &[u8]) -> Result<Vec<u8>, CredentialsError> {
        match self {
            Self::Ed25519(pair) => Ok(pair.sign(message).as_ref().to_vec()),
            Self::Rsa(signer) => {
                let mut signature = vec![0; signer.pair.public().modulus_len()];
                signer
                    .pair
                    .sign(&RSA_PSS_SHA256, &signer.rng, message, &mut signature)
                    .map_err(|_| CredentialsError::Sign {
                        reason: "the RSA key could not sign".to_owned(),
                    })?;
                Ok(signature)
            }
        }
    }
}

/// The label and the still-encoded body of the one PEM block in `text`.
///
/// A literal backslash-n reads as a line break, so a key kept on one line in an env file parses.
fn unarmour(text: &str) -> Result<(String, String), CredentialsError> {
    let text = text.replace("\\n", "\n");
    let mut lines = text.lines().map(str::trim).filter(|line| !line.is_empty());
    let begin = lines.next().ok_or_else(|| pem_error("there is no text"))?;
    let label = begin
        .strip_prefix("-----BEGIN ")
        .and_then(|rest| rest.strip_suffix("-----"))
        .ok_or_else(|| pem_error("the text does not start with a BEGIN line"))?;
    let end = format!("-----END {label}-----");
    let mut body = String::new();
    let mut closed = false;
    for line in lines {
        if closed {
            return Err(pem_error("there is text after the END line"));
        }
        if line == end {
            closed = true;
        } else if line.contains(':') {
            return Err(pem_error(
                "the key is encrypted or carries headers; decrypt it first",
            ));
        } else {
            body.push_str(line);
        }
    }
    if !closed {
        return Err(pem_error("there is no END line for the BEGIN line"));
    }
    Ok((label.to_owned(), body))
}

/// An API key id and the private key that signs for it.
///
/// Build one from the PEM Kalshi issued, with [`from_pem`](Self::from_pem) or
/// [`from_pem_file`](Self::from_pem_file), and hand it to [`PredictionsBuilder::credentials`].
/// The key is held in a [`Secret`], so `Debug` prints the key id and never the key.
#[derive(Clone)]
pub struct Credentials {
    key_id: String,
    key: Secret<Arc<SigningKey>>,
}

impl Credentials {
    /// Reads an Ed25519 or an RSA private key from PEM text.
    ///
    /// Kalshi's keys come as PKCS#8 under `PRIVATE KEY` (Ed25519, or RSA from `openssl genpkey`)
    /// or as PKCS#1 under `RSA PRIVATE KEY`. A literal backslash-n reads as a line break.
    ///
    /// # Errors
    ///
    /// [`CredentialsError`] when the text is not one PEM block, its label is not one of those
    /// two, or `ring` rejects its body. No error quotes the key.
    pub fn from_pem(key_id: impl Into<String>, pem: &str) -> Result<Self, CredentialsError> {
        let key = SigningKey::from_pem(pem)?;
        Ok(Self {
            key_id: key_id.into(),
            key: Secret::new(Arc::new(key)),
        })
    }

    /// Reads the PEM from a file, as [`from_pem`](Self::from_pem) does.
    ///
    /// # Errors
    ///
    /// [`CredentialsError::File`] when the file cannot be read, and every error `from_pem` has.
    pub fn from_pem_file(
        key_id: impl Into<String>,
        path: impl AsRef<Path>,
    ) -> Result<Self, CredentialsError> {
        let path = path.as_ref();
        let pem = std::fs::read_to_string(path).map_err(|error| CredentialsError::File {
            path: path.display().to_string(),
            reason: error.to_string(),
        })?;
        Self::from_pem(key_id, &pem)
    }

    /// The API key id, which Kalshi shows beside the key and which is not a secret.
    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    /// Signs `message` and returns the signature as padded `STANDARD` base64.
    ///
    /// Not part of the API: it exists so a suite can sign a fixed message, which
    /// `sign_request` cannot do because it stamps the clock.
    #[doc(hidden)]
    pub fn sign_message(&self, message: &[u8]) -> Result<String, CredentialsError> {
        Ok(STANDARD.encode(self.key.expose().sign(message)?))
    }

    /// The three headers for a request: the key id, now in milliseconds, and the signature of
    /// `timestamp + method + path`. `path` has no query.
    pub(crate) fn sign_request(
        &self,
        method: &str,
        path: &str,
    ) -> Result<SignedHeaders, CredentialsError> {
        let timestamp = UnixMillis::now().0.to_string();
        let signature = self.sign_message(format!("{timestamp}{method}{path}").as_bytes())?;
        Ok(SignedHeaders {
            key: self.key_id.clone(),
            timestamp,
            signature,
        })
    }
}

/// Written by hand so the key is never printed.
impl fmt::Debug for Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Credentials")
            .field("key_id", &self.key_id)
            .field("key", &self.key)
            .finish()
    }
}

/// The values of the three signing headers for one attempt.
#[derive(Debug, Clone)]
pub(crate) struct SignedHeaders {
    pub(crate) key: String,
    pub(crate) timestamp: String,
    pub(crate) signature: String,
}

impl SignedHeaders {
    /// Each header's name and value, in the order key, timestamp, signature.
    pub(crate) fn pairs(&self) -> [(&'static str, &str); 3] {
        [
            (ACCESS_KEY, &self.key),
            (ACCESS_TIMESTAMP, &self.timestamp),
            (ACCESS_SIGNATURE, &self.signature),
        ]
    }
}

/// Signs each attempt of a request with the account's key.
struct KalshiAuth {
    credentials: Credentials,
}

fn sign_failure(error: CredentialsError) -> ApiError {
    ApiError::Sign(Box::new(error))
}

/// A header value that cannot be sent, as the signing error it is.
fn unfit(name: &str) -> ApiError {
    sign_failure(CredentialsError::Sign {
        reason: format!("the {name} value is not a valid header value"),
    })
}

impl Authenticator for KalshiAuth {
    async fn sign(&self, parts: &mut RequestParts, _attempt: u32) -> Result<(), ApiError> {
        let signed = self
            .credentials
            .sign_request(parts.method.as_str(), &parts.path)
            .map_err(sign_failure)?;
        for (name, value) in signed.pairs() {
            let header = HeaderName::from_bytes(name.as_bytes()).map_err(|_| unfit(name))?;
            let mut value = HeaderValue::from_str(value).map_err(|_| unfit(name))?;
            // Not a secret once it is sent, but not for a log either.
            value.set_sensitive(name == ACCESS_SIGNATURE);
            parts.headers.insert(header, value);
        }
        Ok(())
    }
}

impl PredictionsBuilder {
    /// Sign every request the client sends but its ping with `credentials`.
    ///
    /// Each attempt is signed afresh, so a retry carries a new timestamp. A client built without
    /// credentials signs nothing, and a route that needs a key, such as `account().limits()`,
    /// fails before sending anything.
    pub fn credentials(mut self, credentials: Credentials) -> Self {
        self.auth = Some(DynAuthenticator::new_arc(KalshiAuth { credentials }));
        self
    }
}
