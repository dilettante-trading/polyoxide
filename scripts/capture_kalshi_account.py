#!/usr/bin/env python3
# /// script
# requires-python = ">=3.11"
# dependencies = ["cryptography>=42", "certifi"]
# ///
"""Capture the account-side fixtures of polyoxide-kalshi's `predictions` module.

Run each step as a file with `uv run`, never with `python3 -I`:

    uv run scripts/capture_kalshi_account.py signing DIR
    uv run scripts/capture_kalshi_account.py signing --check DIR
    uv run scripts/capture_kalshi_account.py demo DIR

`signing` needs no account. It makes throwaway keys, an Ed25519 key and an RSA-2048 key, signs one
message with each, and writes DIR/vectors.json and DIR/PROVENANCE.md. The keys exist only for this
fixture, and each is stored as the base64 lines of its PEM with the armour lines left off, so the
file never holds an armour line and no push protection can refuse it; the Rust suite puts the
armour back before it reads a key. It refuses to run when DIR/vectors.json exists, so the keys the
Rust suite reads are never replaced by accident. `signing --check` makes no key and writes
nothing: it re-verifies the files on disk, and exits 1 naming what is wrong.

`demo` reads GET /trade-api/v2/account/limits from the demo host, signed with the owner's demo key,
and writes DIR/limits.json and DIR/PROVENANCE.md. It needs KALSHI_DEMO_API_KEY_ID and
KALSHI_DEMO_PRIVATE_KEY (the PEM text, with a literal backslash-n accepted for a line break), and it
stops without writing anything when either is absent. No fixture is ever written by hand.

`signed_headers(method, path)` is importable: another capture script that needs the demo host's
three KALSHI-ACCESS-* headers imports it from this file's directory. Importing this file runs
nothing: its steps run only under `if __name__ == "__main__":`.
"""

from __future__ import annotations

import argparse
import base64
import json
import os
import time
import urllib.parse
from pathlib import Path

import capture_common
from cryptography.exceptions import InvalidSignature, UnsupportedAlgorithm
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import ed25519, padding, rsa

DEMO_BASE = "https://external-api.demo.kalshi.co"
LIMITS_PATH = "/trade-api/v2/account/limits"
# What every vector signs: a timestamp in milliseconds, the method, and the path.
MESSAGE = "1700000000000GET/trade-api/v2/portfolio/balance"
KEY_ID_VAR = "KALSHI_DEMO_API_KEY_ID"
PEM_VAR = "KALSHI_DEMO_PRIVATE_KEY"

ENCODING = serialization.Encoding
PRIVATE = serialization.PrivateFormat
PUBLIC = serialization.PublicFormat


def b64(data: bytes) -> str:
    return base64.b64encode(data).decode("ascii")


def pss() -> padding.PSS:
    """RSA-PSS as Kalshi asks for it: MGF1 over SHA-256 and a salt as long as the digest."""
    return padding.PSS(mgf=padding.MGF1(hashes.SHA256()), salt_length=padding.PSS.DIGEST_LENGTH)


def sign(key, message: bytes) -> bytes:
    if isinstance(key, ed25519.Ed25519PrivateKey):
        return key.sign(message)
    if isinstance(key, rsa.RSAPrivateKey):
        return key.sign(message, pss(), hashes.SHA256())
    raise SystemExit(f"unsupported key type {type(key).__name__}: Kalshi keys are Ed25519 or RSA")


def body_of(key, fmt) -> str:
    """The base64 lines of `key`'s PEM, joined by newlines, with the armour lines left off."""
    pem = key.private_bytes(ENCODING.PEM, fmt, serialization.NoEncryption()).decode("ascii")
    return "\n".join(line for line in pem.splitlines() if not line.startswith("-----"))


def armour(label: str, body: str) -> bytes:
    """The PEM text around `body`, as the Rust suite builds it before it reads a key."""
    return f"-----BEGIN {label}-----\n{body}\n-----END {label}-----\n".encode("ascii")


def signed_headers(method: str, path: str) -> dict[str, str]:
    """The three KALSHI-ACCESS-* headers for `method path`, signed with the owner's demo key.

    The path is signed without its query. Stops the run, asking for the key, when the key is not
    in the environment.
    """
    key_id = os.environ.get(KEY_ID_VAR, "")
    pem = os.environ.get(PEM_VAR, "")
    if not key_id or not pem:
        raise SystemExit(
            f"{KEY_ID_VAR} and {PEM_VAR} are not both set. This capture needs the owner's demo key: "
            "the key id, and the PEM text (a literal backslash-n is read as a line break). "
            "Stop here and ask the owner; never write the fixture by hand."
        )
    try:
        key = serialization.load_pem_private_key(pem.replace("\\n", "\n").encode(), password=None)
    except (ValueError, TypeError, UnsupportedAlgorithm):
        raise SystemExit(f"{PEM_VAR} does not hold an unencrypted PEM private key") from None
    timestamp = str(int(time.time() * 1000))
    message = f"{timestamp}{method.upper()}{path.split('?')[0]}".encode()
    return {
        "KALSHI-ACCESS-KEY": key_id,
        "KALSHI-ACCESS-TIMESTAMP": timestamp,
        "KALSHI-ACCESS-SIGNATURE": b64(sign(key, message)),
    }


def signing_vectors() -> dict:
    """Throwaway keys, and one signature by each over MESSAGE."""
    message = MESSAGE.encode()
    ed_key = ed25519.Ed25519PrivateKey.generate()
    rsa_key = rsa.generate_private_key(public_exponent=65537, key_size=2048)
    rsa_public = rsa_key.public_key().public_bytes(ENCODING.DER, PUBLIC.PKCS1)
    return {
        "message": MESSAGE,
        "ed25519": {
            "pkcs8_body": body_of(ed_key, PRIVATE.PKCS8),
            "public_key_hex": ed_key.public_key().public_bytes(ENCODING.Raw, PUBLIC.Raw).hex(),
            "signature_b64": b64(ed_key.sign(message)),
        },
        "rsa": {
            "pkcs1_body": body_of(rsa_key, PRIVATE.TraditionalOpenSSL),
            "pkcs8_body": body_of(rsa_key, PRIVATE.PKCS8),
            "public_key_pkcs1_der_b64": b64(rsa_public),
            "pss_signature_b64": b64(rsa_key.sign(message, pss(), hashes.SHA256())),
        },
    }


def check_vectors(vectors: dict) -> list[str]:
    """What is wrong with `vectors`, as sentences that quote no key material."""
    problems: list[str] = []
    if vectors.get("message") != MESSAGE:
        problems.append("message is not the one the Rust suite names")
    message = MESSAGE.encode()

    def verifies(public, signature_b64: str, *scheme) -> bool:
        try:
            public.verify(base64.b64decode(signature_b64), message, *scheme)
        except InvalidSignature:
            return False
        return True

    # The file never holds an armour line, so no push protection can refuse it.
    for section in ("ed25519", "rsa"):
        for name, value in vectors[section].items():
            if "-----" in value:
                problems.append(f"{section}.{name} holds an armour line")
    if problems:
        return problems

    ed = vectors["ed25519"]
    ed_key = serialization.load_pem_private_key(
        armour("PRIVATE KEY", ed["pkcs8_body"]), password=None
    )
    ed_public = ed25519.Ed25519PublicKey.from_public_bytes(bytes.fromhex(ed["public_key_hex"]))
    if ed_key.public_key().public_bytes(ENCODING.Raw, PUBLIC.Raw) != ed_public.public_bytes(
        ENCODING.Raw, PUBLIC.Raw
    ):
        problems.append("ed25519.public_key_hex is not the private key's public key")
    if not verifies(ed_public, ed["signature_b64"]):
        problems.append("ed25519.signature_b64 does not verify under ed25519.public_key_hex")

    rs = vectors["rsa"]
    # The label decides how a body is read: a PKCS#8 body under RSA PRIVATE KEY would not load.
    pkcs1 = serialization.load_pem_private_key(
        armour("RSA PRIVATE KEY", rs["pkcs1_body"]), password=None
    )
    pkcs8 = serialization.load_pem_private_key(
        armour("PRIVATE KEY", rs["pkcs8_body"]), password=None
    )
    public = pkcs1.public_key()
    if public.public_numbers() != pkcs8.public_key().public_numbers():
        problems.append("rsa.pkcs1_body and rsa.pkcs8_body are not one key")
    if public.public_bytes(ENCODING.DER, PUBLIC.PKCS1) != base64.b64decode(
        rs["public_key_pkcs1_der_b64"]
    ):
        problems.append("rsa.public_key_pkcs1_der_b64 is not the private key's public key")
    if not verifies(public, rs["pss_signature_b64"], pss(), hashes.SHA256()):
        problems.append("rsa.pss_signature_b64 does not verify under the public key")
    return problems


def generate(out: Path) -> None:
    target = out / "vectors.json"
    if target.exists():
        raise SystemExit(f"{target} exists. Delete it to make new keys; the Rust suite reads it.")
    vectors = signing_vectors()
    problems = check_vectors(vectors)
    if problems:
        raise SystemExit("the vectors just made are wrong: " + "; ".join(problems))
    capture_common.write_json(target, vectors)
    what = "throwaway Ed25519 and RSA-2048 keys, and one signature by each"
    rows = [["vectors.json", what, capture_common.stamp()]]
    capture_common.write_provenance(
        out,
        [
            "# Provenance",
            "",
            "`vectors.json` is made by `scripts/capture_kalshi_account.py signing`. Its keys are "
            "throwaway ones that exist only for this fixture: no Kalshi account has ever held them. "
            f"Every signature is over `{MESSAGE}`.",
            "",
            "The file never holds an armour line, so no push protection can refuse it. Each "
            "`*_body` is the base64 lines of a PEM, joined by newlines. The Rust suite puts the "
            "armour back before it reads a key, with the label `PRIVATE KEY` around a "
            "`pkcs8_body` and `RSA PRIVATE KEY` around the `pkcs1_body`.",
            "",
            *capture_common.provenance_table(["File", "What", "Generated"], rows),
            "",
            "Check it with `uv run scripts/capture_kalshi_account.py signing --check "
            "polyoxide-kalshi/tests/fixtures/predictions/signing`.",
        ],
    )
    print(f"wrote {target}")


def check(out: Path) -> None:
    target = out / "vectors.json"
    if not target.exists():
        raise SystemExit(f"{target} is missing: run the signing step without --check first")
    problems = check_vectors(json.loads(target.read_text()))
    if problems:
        print(f"{target}:")
        for problem in problems:
            print(f"  {problem}")
        raise SystemExit(1)
    print(f"signing vectors ok: {target}")


def demo(out: Path) -> None:
    def headers(method: str, url: str) -> dict[str, str]:
        return signed_headers(method, urllib.parse.urlparse(url).path)

    reply = capture_common.get(DEMO_BASE + LIMITS_PATH, headers=headers)
    body = capture_common.require_ok(reply, f"GET {LIMITS_PATH} on the demo host")
    capture_common.write_json(out / "limits.json", body)
    rows = [["limits.json", f"GET {LIMITS_PATH}", "the demo host", "yes", capture_common.stamp()]]
    capture_common.write_provenance(
        out,
        [
            "# Provenance",
            "",
            "`limits.json` is the body of `GET /trade-api/v2/account/limits` as the demo host "
            f"(`{DEMO_BASE}`) served it, signed with the owner's demo key ({KEY_ID_VAR} and "
            f"{PEM_VAR}). Only its indentation was changed.",
            "",
            *capture_common.provenance_table(["File", "Route", "Host", "Signed", "Captured"], rows),
            "",
            "Capture it again with `uv run scripts/capture_kalshi_account.py demo "
            "polyoxide-kalshi/tests/fixtures/predictions/account`.",
        ],
    )
    print(f"wrote {out / 'limits.json'}")


def main(argv: list[str] | None = None) -> None:
    parser = argparse.ArgumentParser(description=(__doc__ or "").split("\n\n")[0])
    steps = parser.add_subparsers(dest="step", required=True)
    signing_step = steps.add_parser("signing", help="throwaway keys, and one signature by each")
    signing_step.add_argument("out", type=Path, help="the signing fixtures' directory")
    signing_step.add_argument("--check", action="store_true", help="re-verify the fixture on disk")
    demo_step = steps.add_parser("demo", help="the demo host's /account/limits, signed")
    demo_step.add_argument("out", type=Path, help="the account fixtures' directory")
    args = parser.parse_args(argv)
    if args.step == "demo":
        demo(args.out)
    elif args.check:
        check(args.out)
    else:
        generate(args.out)


if __name__ == "__main__":
    main()
