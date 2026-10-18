//! Signing: the vectors, both key types, the PEM forms Kalshi hands out, and what a refusal says.
//!
//! The keys in `tests/fixtures/predictions/signing/vectors.json` are throwaway ones, made by
//! `scripts/capture_kalshi_account.py signing`. No account has ever held them. The file holds each
//! key as the base64 lines of its PEM with no armour line, and `armour` puts the armour back.

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use polyoxide_kalshi::predictions::{Credentials, CredentialsError};
use polyoxide_test_support::fixtures;
use polyoxide_venue::{Class, Classify};
use ring::signature::{UnparsedPublicKey, ED25519, RSA_PSS_2048_8192_SHA256};
use serde_json::Value;

/// The message the vectors sign: a timestamp in milliseconds, the method and the path.
const MESSAGE: &str = "1700000000000GET/trade-api/v2/portfolio/balance";

fn vectors() -> Value {
    fixtures!("predictions/signing").json("vectors")
}

/// A string field of the vectors.
fn field(vectors: &Value, key: &str, name: &str) -> String {
    vectors[key][name]
        .as_str()
        .unwrap_or_else(|| panic!("vectors.json has no {key}.{name}"))
        .to_owned()
}

/// The vectors' Ed25519 key as PEM.
fn ed25519_pem() -> String {
    armour("PRIVATE KEY", &field(&vectors(), "ed25519", "pkcs8_body"))
}

fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16).expect("the vectors hold valid hex"))
        .collect()
}

fn base64(text: &str) -> Vec<u8> {
    STANDARD
        .decode(text)
        .expect("the vectors hold valid base64")
}

fn rsa_public_key(vectors: &Value) -> Vec<u8> {
    base64(&field(vectors, "rsa", "public_key_pkcs1_der_b64"))
}

fn verifies_pss(public_key: &[u8], message: &[u8], signature: &[u8]) -> bool {
    UnparsedPublicKey::new(&RSA_PSS_2048_8192_SHA256, public_key)
        .verify(message, signature)
        .is_ok()
}

/// A refusal is the caller's: `InvalidRequest`, which no retry fixes.
fn assert_invalid_request(error: &CredentialsError) {
    assert_eq!(error.class(), Class::InvalidRequest, "{error:?}");
    assert!(!error.is_retriable(), "{error:?}");
}

/// A PEM block around `body`: a vectors body, or a made-up one for a refusal.
fn armour(label: &str, body: &str) -> String {
    format!("-----BEGIN {label}-----\n{body}\n-----END {label}-----\n")
}

#[test]
fn the_ed25519_signature_is_the_vectors() {
    let vectors = vectors();
    assert_eq!(
        vectors["message"], MESSAGE,
        "the vectors sign the message this suite names"
    );
    let credentials = Credentials::from_pem("key", &ed25519_pem()).unwrap();

    assert_eq!(
        credentials.sign_message(MESSAGE.as_bytes()).unwrap(),
        field(&vectors, "ed25519", "signature_b64"),
        "Ed25519 is deterministic: one key and message give one signature"
    );
}

#[test]
fn the_vectors_ed25519_signature_verifies_under_its_public_key() {
    let vectors = vectors();
    let public_key = hex(&field(&vectors, "ed25519", "public_key_hex"));
    let signature = base64(&field(&vectors, "ed25519", "signature_b64"));

    UnparsedPublicKey::new(&ED25519, public_key)
        .verify(MESSAGE.as_bytes(), &signature)
        .expect("the vectors' signature verifies under the vectors' public key");
}

#[test]
fn ring_verifies_pythons_pss_signature_so_mgf1_and_the_salt_length_agree() {
    let vectors = vectors();
    let signature = base64(&field(&vectors, "rsa", "pss_signature_b64"));

    assert!(verifies_pss(
        &rsa_public_key(&vectors),
        MESSAGE.as_bytes(),
        &signature
    ));
}

#[test]
fn the_rsa_signature_verifies_for_both_pem_forms_and_only_for_its_message() {
    let vectors = vectors();
    let public_key = rsa_public_key(&vectors);

    let forms = [
        ("pkcs1_body", "RSA PRIVATE KEY"),
        ("pkcs8_body", "PRIVATE KEY"),
    ];
    for (form, label) in forms {
        let pem = armour(label, &field(&vectors, "rsa", form));
        let credentials = Credentials::from_pem("key", &pem).unwrap();
        let signature = base64(&credentials.sign_message(MESSAGE.as_bytes()).unwrap());

        assert!(
            verifies_pss(&public_key, MESSAGE.as_bytes(), &signature),
            "{form}"
        );
        assert!(
            !verifies_pss(&public_key, b"another message", &signature),
            "{form}"
        );
    }
}

#[test]
fn rsa_pss_is_salted_so_two_signatures_of_one_message_differ() {
    let vectors = vectors();
    let pem = armour("RSA PRIVATE KEY", &field(&vectors, "rsa", "pkcs1_body"));
    let credentials = Credentials::from_pem("key", &pem).unwrap();

    let first = credentials.sign_message(MESSAGE.as_bytes()).unwrap();
    let second = credentials.sign_message(MESSAGE.as_bytes()).unwrap();

    assert_ne!(first, second);
}

#[test]
fn a_key_on_one_line_with_literal_backslash_n_signs_as_the_pem_does() {
    let one_line = ed25519_pem().trim_end().replace('\n', "\\n");
    assert!(!one_line.contains('\n'), "the test's own premise");

    let credentials = Credentials::from_pem("key", &one_line).unwrap();

    assert_eq!(
        credentials.sign_message(MESSAGE.as_bytes()).unwrap(),
        field(&vectors(), "ed25519", "signature_b64")
    );
}

#[test]
fn windows_line_endings_and_blank_space_around_the_pem_parse() {
    let padded = format!("\n  {}\n\n", ed25519_pem().replace('\n', "\r\n"));

    let credentials = Credentials::from_pem("key", &padded).unwrap();

    assert_eq!(
        credentials.sign_message(MESSAGE.as_bytes()).unwrap(),
        field(&vectors(), "ed25519", "signature_b64")
    );
}

#[test]
fn an_ec_key_is_refused_by_its_label_alone() {
    let pem = armour("EC PRIVATE KEY", "c2VjcmV0LWVjLWJvZHk=");

    let error = Credentials::from_pem("key", &pem).unwrap_err();

    assert!(matches!(&error, CredentialsError::Label { label } if label == "EC PRIVATE KEY"));
    assert!(error.to_string().contains("EC PRIVATE KEY"), "{error}");
    assert!(!error.to_string().contains("c2VjcmV0"), "{error}");
    assert_invalid_request(&error);
}

#[test]
fn an_encrypted_key_is_refused_by_its_label() {
    let pem = armour("ENCRYPTED PRIVATE KEY", "c2VjcmV0LWVuY3J5cHRlZA==");

    let error = Credentials::from_pem("key", &pem).unwrap_err();

    assert!(
        matches!(&error, CredentialsError::Label { label } if label == "ENCRYPTED PRIVATE KEY")
    );
    assert!(!error.to_string().contains("c2VjcmV0"), "{error}");
    assert_invalid_request(&error);
}

#[test]
fn a_legacy_encrypted_pem_is_refused_before_its_body_is_read() {
    let pem = "-----BEGIN RSA PRIVATE KEY-----\nProc-Type: 4,ENCRYPTED\nDEK-Info: AES-128-CBC,00\n\nc2VjcmV0\n-----END RSA PRIVATE KEY-----\n";

    let error = Credentials::from_pem("key", pem).unwrap_err();

    assert!(matches!(error, CredentialsError::Pem { .. }), "{error:?}");
    assert!(error.to_string().contains("encrypted"), "{error}");
    assert!(!error.to_string().contains("c2VjcmV0"), "{error}");
    assert_invalid_request(&error);
}

#[test]
fn a_body_that_is_not_base64_is_a_pem_error_that_does_not_quote_it() {
    let pem = armour("PRIVATE KEY", "c2VjcmV0-body!!not base64");

    let error = Credentials::from_pem("key", &pem).unwrap_err();

    assert!(matches!(error, CredentialsError::Pem { .. }), "{error:?}");
    assert!(!error.to_string().contains("c2VjcmV0"), "{error}");
    assert_invalid_request(&error);
}

#[test]
fn a_body_that_is_base64_but_no_key_is_a_key_error_that_does_not_quote_it() {
    let body = STANDARD.encode(b"polyoxide-test: this is not a private key");

    for label in ["PRIVATE KEY", "RSA PRIVATE KEY"] {
        let error = Credentials::from_pem("key", &armour(label, &body)).unwrap_err();

        assert!(
            matches!(error, CredentialsError::Key { .. }),
            "{label}: {error:?}"
        );
        assert!(!error.to_string().contains(&body[..16]), "{label}: {error}");
        assert!(
            !error.to_string().contains("polyoxide-test"),
            "{label}: {error}"
        );
        assert_invalid_request(&error);
    }
}

#[test]
fn text_that_is_not_one_armoured_block_is_a_pem_error() {
    let texts = [
        "",
        "not a key",
        "-----BEGIN PRIVATE KEY-----\nAAAA\n",
        "-----BEGIN PRIVATE KEY-----\nAAAA\n-----END PRIVATE KEY-----\nextra",
    ];

    for text in texts {
        let error = Credentials::from_pem("key", text).unwrap_err();

        assert!(
            matches!(error, CredentialsError::Pem { .. }),
            "{text:?}: {error:?}"
        );
        assert_invalid_request(&error);
    }
}

#[test]
fn debug_prints_the_key_id_and_no_key_material() {
    let pem = ed25519_pem();
    let body_line = pem.lines().nth(1).expect("a PEM has a body line");
    let credentials = Credentials::from_pem("my-key-id", &pem).unwrap();

    for printed in [format!("{credentials:?}"), format!("{credentials:#?}")] {
        assert!(printed.contains("my-key-id"), "{printed}");
        assert!(printed.contains("Secret([REDACTED])"), "{printed}");
        assert!(!printed.contains(body_line), "{printed}");
    }
}

#[test]
fn from_pem_file_reads_a_key_and_names_the_path_of_one_it_cannot() {
    let dir = std::env::temp_dir().join(format!("polyoxide-kalshi-signing-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("key.pem");
    std::fs::write(&path, ed25519_pem()).unwrap();

    let credentials = Credentials::from_pem_file("file-key", path).unwrap();
    assert_eq!(credentials.key_id(), "file-key");
    assert_eq!(
        credentials.sign_message(MESSAGE.as_bytes()).unwrap(),
        field(&vectors(), "ed25519", "signature_b64")
    );

    let missing = dir.join("missing.pem");
    let error = Credentials::from_pem_file("file-key", &missing).unwrap_err();
    assert!(
        matches!(&error, CredentialsError::File { path, .. } if *path == missing.display().to_string()),
        "{error:?}"
    );
    assert_invalid_request(&error);

    std::fs::remove_dir_all(dir).unwrap();
}
