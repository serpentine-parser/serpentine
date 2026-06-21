"""Tests for build_jwt_verifier() and JWTVerifier.verify_token()."""

import time
import asyncio

import jwt
import pytest
from cryptography.hazmat.backends import default_backend
from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric import rsa

from serpentine.mcp.auth import ConfigError, JWTVerifier, build_jwt_verifier


# ---------------------------------------------------------------------------
# Fixtures — RSA key pair
# ---------------------------------------------------------------------------

@pytest.fixture(scope="module")
def rsa_private_key():
    return rsa.generate_private_key(
        public_exponent=65537, key_size=2048, backend=default_backend()
    )


@pytest.fixture(scope="module")
def rsa_public_pem(rsa_private_key):
    return rsa_private_key.public_key().public_bytes(
        encoding=serialization.Encoding.PEM,
        format=serialization.PublicFormat.SubjectPublicKeyInfo,
    ).decode()


@pytest.fixture(scope="module")
def rsa_private_pem(rsa_private_key):
    return rsa_private_key.private_bytes(
        encoding=serialization.Encoding.PEM,
        format=serialization.PrivateFormat.TraditionalOpenSSL,
        encryption_algorithm=serialization.NoEncryption(),
    ).decode()


def _make_token(private_pem: str, *, sub="user1", iss="test-issuer", aud="test-aud",
                exp_offset=300, extra=None) -> str:
    payload = {
        "sub": sub,
        "iss": iss,
        "aud": aud,
        "iat": int(time.time()),
        "exp": int(time.time()) + exp_offset,
    }
    if extra:
        payload.update(extra)
    return jwt.encode(payload, private_pem, algorithm="RS256")


def _make_verifier(public_pem, *, issuer="test-issuer", audience="test-aud") -> JWTVerifier:
    return JWTVerifier(
        jwks_uri=None,
        public_key=public_pem,
        issuer=issuer,
        audience=audience,
    )


# ---------------------------------------------------------------------------
# build_jwt_verifier() — startup guard
# ---------------------------------------------------------------------------

def test_no_key_no_unauthenticated_raises(monkeypatch):
    monkeypatch.delenv("SERPENTINE_JWKS_URI", raising=False)
    monkeypatch.delenv("SERPENTINE_JWT_PUBLIC_KEY", raising=False)
    monkeypatch.delenv("SERPENTINE_MCP_ALLOW_UNAUTHENTICATED", raising=False)
    with pytest.raises(ConfigError, match="SERPENTINE_JWKS_URI"):
        build_jwt_verifier()


def test_allow_unauthenticated_returns_none(monkeypatch):
    monkeypatch.delenv("SERPENTINE_JWKS_URI", raising=False)
    monkeypatch.delenv("SERPENTINE_JWT_PUBLIC_KEY", raising=False)
    monkeypatch.setenv("SERPENTINE_MCP_ALLOW_UNAUTHENTICATED", "true")
    assert build_jwt_verifier() is None


def test_allow_unauthenticated_false_without_key_raises(monkeypatch):
    monkeypatch.delenv("SERPENTINE_JWKS_URI", raising=False)
    monkeypatch.delenv("SERPENTINE_JWT_PUBLIC_KEY", raising=False)
    monkeypatch.setenv("SERPENTINE_MCP_ALLOW_UNAUTHENTICATED", "false")
    with pytest.raises(ConfigError):
        build_jwt_verifier()


def test_missing_issuer_raises(monkeypatch, rsa_public_pem):
    monkeypatch.delenv("SERPENTINE_JWKS_URI", raising=False)
    monkeypatch.delenv("SERPENTINE_MCP_ALLOW_UNAUTHENTICATED", raising=False)
    monkeypatch.setenv("SERPENTINE_JWT_PUBLIC_KEY", rsa_public_pem)
    monkeypatch.setenv("SERPENTINE_JWT_AUDIENCE", "myapp")
    monkeypatch.delenv("SERPENTINE_JWT_ISSUER", raising=False)
    with pytest.raises(ConfigError, match="SERPENTINE_JWT_ISSUER"):
        build_jwt_verifier()


def test_missing_audience_raises(monkeypatch, rsa_public_pem):
    monkeypatch.delenv("SERPENTINE_JWKS_URI", raising=False)
    monkeypatch.delenv("SERPENTINE_MCP_ALLOW_UNAUTHENTICATED", raising=False)
    monkeypatch.setenv("SERPENTINE_JWT_PUBLIC_KEY", rsa_public_pem)
    monkeypatch.setenv("SERPENTINE_JWT_ISSUER", "https://issuer.example.com")
    monkeypatch.delenv("SERPENTINE_JWT_AUDIENCE", raising=False)
    with pytest.raises(ConfigError, match="SERPENTINE_JWT_AUDIENCE"):
        build_jwt_verifier()


def test_static_public_key_returns_verifier(monkeypatch, rsa_public_pem):
    monkeypatch.delenv("SERPENTINE_JWKS_URI", raising=False)
    monkeypatch.delenv("SERPENTINE_MCP_ALLOW_UNAUTHENTICATED", raising=False)
    monkeypatch.setenv("SERPENTINE_JWT_PUBLIC_KEY", rsa_public_pem)
    monkeypatch.setenv("SERPENTINE_JWT_ISSUER", "test-issuer")
    monkeypatch.setenv("SERPENTINE_JWT_AUDIENCE", "test-aud")
    verifier = build_jwt_verifier()
    assert isinstance(verifier, JWTVerifier)


def test_jwks_uri_returns_verifier(monkeypatch):
    monkeypatch.delenv("SERPENTINE_JWT_PUBLIC_KEY", raising=False)
    monkeypatch.delenv("SERPENTINE_MCP_ALLOW_UNAUTHENTICATED", raising=False)
    monkeypatch.setenv("SERPENTINE_JWKS_URI", "https://example.com/.well-known/jwks.json")
    monkeypatch.setenv("SERPENTINE_JWT_ISSUER", "test-issuer")
    monkeypatch.setenv("SERPENTINE_JWT_AUDIENCE", "test-aud")
    verifier = build_jwt_verifier()
    assert isinstance(verifier, JWTVerifier)


# ---------------------------------------------------------------------------
# JWTVerifier.verify_token() — valid token
# ---------------------------------------------------------------------------

def test_valid_token_returns_access_token(rsa_public_pem, rsa_private_pem):
    verifier = _make_verifier(rsa_public_pem)
    token = _make_token(rsa_private_pem)
    result = asyncio.run(verifier.verify_token(token))
    assert result is not None
    assert result.client_id == "user1"


def test_access_token_carries_claims(rsa_public_pem, rsa_private_pem):
    verifier = _make_verifier(rsa_public_pem)
    token = _make_token(rsa_private_pem, extra={"scope": "read write"})
    result = asyncio.run(verifier.verify_token(token))
    assert result is not None
    assert "read" in result.scopes
    assert "write" in result.scopes


# ---------------------------------------------------------------------------
# JWTVerifier.verify_token() — security: reject invalid tokens
# ---------------------------------------------------------------------------

def test_expired_token_rejected(rsa_public_pem, rsa_private_pem):
    verifier = _make_verifier(rsa_public_pem)
    token = _make_token(rsa_private_pem, exp_offset=-1)
    assert asyncio.run(verifier.verify_token(token)) is None


def test_wrong_audience_rejected(rsa_public_pem, rsa_private_pem):
    verifier = _make_verifier(rsa_public_pem, audience="correct-aud")
    token = _make_token(rsa_private_pem, aud="wrong-aud")
    assert asyncio.run(verifier.verify_token(token)) is None


def test_wrong_issuer_rejected(rsa_public_pem, rsa_private_pem):
    verifier = _make_verifier(rsa_public_pem, issuer="correct-issuer")
    token = _make_token(rsa_private_pem, iss="evil-issuer")
    assert asyncio.run(verifier.verify_token(token)) is None


def test_tampered_signature_rejected(rsa_public_pem, rsa_private_pem):
    verifier = _make_verifier(rsa_public_pem)
    token = _make_token(rsa_private_pem)
    # flip a byte in the signature (last segment)
    header, payload, sig = token.rsplit(".", 2)
    bad_sig = sig[:-4] + ("AAAA" if not sig.endswith("AAAA") else "BBBB")
    bad_token = f"{header}.{payload}.{bad_sig}"
    assert asyncio.run(verifier.verify_token(bad_token)) is None


def test_wrong_key_rejected(rsa_public_pem, rsa_private_pem):
    other_key = rsa.generate_private_key(
        public_exponent=65537, key_size=2048, backend=default_backend()
    )
    other_private_pem = other_key.private_bytes(
        encoding=serialization.Encoding.PEM,
        format=serialization.PrivateFormat.TraditionalOpenSSL,
        encryption_algorithm=serialization.NoEncryption(),
    ).decode()
    verifier = _make_verifier(rsa_public_pem)
    token = _make_token(other_private_pem)
    assert asyncio.run(verifier.verify_token(token)) is None


def test_garbage_token_rejected(rsa_public_pem):
    verifier = _make_verifier(rsa_public_pem)
    assert asyncio.run(verifier.verify_token("not.a.token")) is None
