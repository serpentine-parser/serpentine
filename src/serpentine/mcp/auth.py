import logging
import os

import jwt
from fastmcp.server.auth import TokenVerifier
from mcp.server.auth.provider import AccessToken

logger = logging.getLogger(__name__)


class ConfigError(Exception):
    pass


class JWTVerifier(TokenVerifier):
    """Verifies Bearer JWTs using JWKS or a static RSA public key."""

    def __init__(
        self,
        *,
        jwks_uri: str | None,
        public_key: str | None,
        issuer: str | None,
        audience: str | None,
    ) -> None:
        super().__init__()
        self._jwks_client: jwt.PyJWKClient | None = None
        self._public_key = public_key
        self._issuer = issuer
        self._audience = audience

        if jwks_uri:
            self._jwks_client = jwt.PyJWKClient(jwks_uri)

    async def verify_token(self, token: str) -> AccessToken | None:
        try:
            if self._jwks_client is not None:
                signing_key = self._jwks_client.get_signing_key_from_jwt(token)
                key = signing_key.key
            else:
                key = self._public_key

            options: dict = {"verify_exp": True}
            if self._audience is None:
                options["verify_aud"] = False

            claims = jwt.decode(
                token,
                key,
                algorithms=["RS256", "RS384", "RS512", "ES256", "ES384", "ES512"],
                issuer=self._issuer,
                audience=self._audience,
                options=options,
            )
        except jwt.ExpiredSignatureError:
            logger.debug("JWT rejected: expired")
            return None
        except jwt.InvalidTokenError as exc:
            logger.debug("JWT rejected: %s", exc)
            return None

        subject = claims.get("sub", claims.get("client_id", ""))
        expires_at = claims.get("exp")

        return AccessToken(
            token=token,
            client_id=str(subject),
            scopes=claims.get("scope", "").split() if isinstance(claims.get("scope"), str) else [],
            expires_at=int(expires_at) if expires_at is not None else None,
            subject=str(subject),
            claims=claims,
        )


def build_jwt_verifier() -> JWTVerifier | None:
    """
    Build a JWTVerifier from environment variables, or return None if auth is
    explicitly disabled. Raises ConfigError on invalid configuration.
    """
    allow_unauth = os.environ.get("SERPENTINE_MCP_ALLOW_UNAUTHENTICATED", "").lower() == "true"

    jwks_uri = os.environ.get("SERPENTINE_JWKS_URI")
    public_key = os.environ.get("SERPENTINE_JWT_PUBLIC_KEY")
    issuer = os.environ.get("SERPENTINE_JWT_ISSUER")
    audience = os.environ.get("SERPENTINE_JWT_AUDIENCE")

    if not jwks_uri and not public_key:
        if allow_unauth:
            logger.warning(
                "WARNING: SERPENTINE_MCP_ALLOW_UNAUTHENTICATED=true — "
                "JWT authentication is DISABLED. Do not use this in production."
            )
            return None
        raise ConfigError(
            "Auth configuration missing: set SERPENTINE_JWKS_URI or SERPENTINE_JWT_PUBLIC_KEY. "
            "To disable auth for local development only, set SERPENTINE_MCP_ALLOW_UNAUTHENTICATED=true."
        )

    if not issuer or not audience:
        raise ConfigError(
            "SERPENTINE_JWT_ISSUER and SERPENTINE_JWT_AUDIENCE are required when auth is enabled. "
            "Omitting either accepts tokens from any issuer or audience on the configured key, "
            "enabling cross-application token reuse attacks."
        )

    return JWTVerifier(
        jwks_uri=jwks_uri,
        public_key=public_key,
        issuer=issuer,
        audience=audience,
    )
