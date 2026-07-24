//! Stateless access token (JWT HS256) and refresh token issuance.

use base64ct::{Base64UrlUnpadded, Encoding};
use domain_identity::session::RefreshToken;
use domain_identity::token::AccessTokenClaims;
use foundation::{
    Clock, ErrorCode, PlatformError, RandomSource, TenantId, UserId, UtcTimestamp, uuid::Uuid,
};
use hmac::{Hmac, KeyInit, Mac};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

const MAX_ISSUER_LEN: usize = 256;
const MAX_AUDIENCE_LEN: usize = 256;
const MAX_JTI_LEN: usize = 256;

/// Issues and verifies HMAC-SHA256 access tokens and generates refresh tokens.
#[derive(Clone)]
pub struct TokenService {
    secret: Vec<u8>,
    issuer: String,
    audience: String,
    access_ttl_seconds: i64,
}

impl std::fmt::Debug for TokenService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenService")
            .field("secret", &"<redacted>")
            .field("issuer", &self.issuer)
            .field("audience", &self.audience)
            .field("access_ttl_seconds", &self.access_ttl_seconds)
            .finish()
    }
}

impl TokenService {
    /// Create a token service. `secret` must be at least 32 bytes and
    /// `access_ttl_seconds` must be positive.
    pub fn new(
        secret: impl AsRef<[u8]>,
        issuer: impl AsRef<str>,
        audience: impl AsRef<str>,
        access_ttl_seconds: i64,
    ) -> Result<Self, PlatformError> {
        let secret_bytes = secret.as_ref();
        if secret_bytes.len() < 32 {
            return Err(PlatformError::new(
                ErrorCode::Invalid,
                "token secret must be at least 32 bytes",
            ));
        }
        if access_ttl_seconds <= 0 {
            return Err(PlatformError::new(
                ErrorCode::Invalid,
                "access token ttl must be positive",
            ));
        }
        let issuer = issuer.as_ref();
        validate_token_string(issuer, "issuer", MAX_ISSUER_LEN)?;
        let audience = audience.as_ref();
        validate_token_string(audience, "audience", MAX_AUDIENCE_LEN)?;
        Ok(Self {
            secret: secret_bytes.to_vec(),
            issuer: issuer.to_string(),
            audience: audience.to_string(),
            access_ttl_seconds,
        })
    }

    /// Issue a fresh access token for the given subject and session.
    pub fn issue_access_token(
        &self,
        user_id: UserId,
        tenant_id: TenantId,
        session_version: u64,
        now: UtcTimestamp,
        jti: impl AsRef<str>,
    ) -> Result<String, PlatformError> {
        let jti = jti.as_ref();
        validate_token_string(jti, "jti", MAX_JTI_LEN)?;
        let iat = now.timestamp_millis() / 1000;
        let exp = iat
            .checked_add(self.access_ttl_seconds)
            .ok_or_else(|| PlatformError::new(ErrorCode::Invalid, "token expiration overflow"))?;
        let claims = AccessTokenClaims {
            sub: user_id,
            tenant_id,
            session_version,
            aud: self.audience.clone(),
            iss: self.issuer.clone(),
            nbf: iat,
            exp,
            jti: jti.to_string(),
        };
        self.sign(&claims)
    }

    /// Verify an access token, including its session version.
    pub fn verify_access_token(
        &self,
        token: &str,
        now: UtcTimestamp,
        expected_session_version: u64,
    ) -> Result<AccessTokenClaims, PlatformError> {
        let claims = self.verify_access_token_claims(token, now)?;
        if claims.session_version != expected_session_version {
            return Err(PlatformError::new(
                ErrorCode::Unauthenticated,
                "invalid token",
            ));
        }
        Ok(claims)
    }

    /// Verify an access token's signature, header, issuer, audience, expiration,
    /// not-before time and jti claims, returning the decoded claims. The caller is
    /// still responsible for checking the session version against the stored user
    /// record, so this can be used before fetching the user.
    pub fn verify_access_token_claims(
        &self,
        token: &str,
        now: UtcTimestamp,
    ) -> Result<AccessTokenClaims, PlatformError> {
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 3 {
            return Err(PlatformError::new(
                ErrorCode::Unauthenticated,
                "invalid token",
            ));
        }

        let header_b64 = parts[0];
        let claims_b64 = parts[1];
        let sig_b64 = parts[2];

        self.verify_header(header_b64)?;

        // Verify the signature before decoding or validating claims so that
        // an attacker cannot probe claim validation with a forged token.
        let signature = Base64UrlUnpadded::decode_vec(sig_b64)
            .map_err(|_| PlatformError::new(ErrorCode::Unauthenticated, "invalid token"))?;
        let message = format!("{}.{}", header_b64, claims_b64);
        let mut mac = new_mac(&self.secret)?;
        mac.update(message.as_bytes());
        mac.verify_slice(&signature)
            .map_err(|_| PlatformError::new(ErrorCode::Unauthenticated, "invalid token"))?;

        let claims_bytes = Base64UrlUnpadded::decode_vec(claims_b64)
            .map_err(|_| PlatformError::new(ErrorCode::Unauthenticated, "invalid token"))?;
        let claims: AccessTokenClaims = serde_json::from_slice(&claims_bytes)
            .map_err(|_| PlatformError::new(ErrorCode::Unauthenticated, "invalid token"))?;

        claims.validate(&self.issuer, &self.audience, now)?;
        self.validate_jti(&claims)?;

        Ok(claims)
    }

    fn verify_header(&self, header_b64: &str) -> Result<(), PlatformError> {
        let header_bytes = Base64UrlUnpadded::decode_vec(header_b64)
            .map_err(|_| PlatformError::new(ErrorCode::Unauthenticated, "invalid token"))?;
        let header: serde_json::Value = serde_json::from_slice(&header_bytes)
            .map_err(|_| PlatformError::new(ErrorCode::Unauthenticated, "invalid token"))?;
        let alg = header.get("alg").and_then(|v| v.as_str()).unwrap_or("");
        let typ = header.get("typ").and_then(|v| v.as_str()).unwrap_or("");
        if alg != "HS256" || typ != "JWT" {
            return Err(PlatformError::new(
                ErrorCode::Unauthenticated,
                "invalid token",
            ));
        }
        Ok(())
    }

    fn validate_jti(&self, claims: &AccessTokenClaims) -> Result<(), PlatformError> {
        if claims.jti.is_empty() {
            return Err(PlatformError::new(
                ErrorCode::Unauthenticated,
                "invalid token",
            ));
        }
        Ok(())
    }

    /// Generate a new raw refresh token and its stored representation.
    #[allow(clippy::too_many_arguments)]
    pub fn generate_refresh_token(
        &self,
        tenant_id: TenantId,
        user_id: UserId,
        family_id: Uuid,
        session_version: u64,
        expires_at: UtcTimestamp,
        random: &dyn RandomSource,
        clock: &dyn Clock,
    ) -> Result<(String, RefreshToken), PlatformError> {
        let mut raw = [0u8; 32];
        random.fill_bytes(&mut raw)?;
        let raw = Base64UrlUnpadded::encode_string(&raw);
        let token_hash = hash_raw(&raw);

        let id = foundation::generate_uuid(clock, random)?;

        let created_at = clock.now();
        let token = RefreshToken::from_parts(
            id,
            tenant_id,
            user_id,
            family_id,
            token_hash,
            session_version,
            false,
            expires_at,
            created_at,
        )?;
        Ok((raw, token))
    }

    /// Return the hash used to look up a raw refresh token.
    pub fn hash_refresh_token(raw: &str) -> String {
        hash_raw(raw)
    }

    fn sign(&self, claims: &AccessTokenClaims) -> Result<String, PlatformError> {
        let header = br#"{"alg":"HS256","typ":"JWT"}"#;
        let header_b64 = Base64UrlUnpadded::encode_string(header);
        let claims_json = serde_json::to_string(claims)
            .map_err(|e| PlatformError::invalid("claims", e.to_string()))?;
        let claims_b64 = Base64UrlUnpadded::encode_string(claims_json.as_bytes());

        let message = format!("{}.{}", header_b64, claims_b64);
        let mut mac = new_mac(&self.secret)?;
        mac.update(message.as_bytes());
        let sig = mac.finalize().into_bytes();
        let sig_b64 = Base64UrlUnpadded::encode_string(sig.as_ref());

        Ok(format!("{}.{}.{}", header_b64, claims_b64, sig_b64))
    }
}

fn new_mac(secret: &[u8]) -> Result<HmacSha256, PlatformError> {
    HmacSha256::new_from_slice(secret)
        .map_err(|_| PlatformError::new(ErrorCode::Invalid, "invalid token secret"))
}

fn hash_raw(raw: &str) -> String {
    let hash = Sha256::digest(raw.as_bytes());
    Base64UrlUnpadded::encode_string(&hash)
}

fn validate_token_string(value: &str, field: &str, max: usize) -> Result<(), PlatformError> {
    if value.trim().is_empty() {
        return Err(PlatformError::invalid(field, "value must not be empty"));
    }
    if value.len() > max {
        return Err(PlatformError::invalid(
            field,
            "value exceeds maximum length",
        ));
    }
    Ok(())
}
