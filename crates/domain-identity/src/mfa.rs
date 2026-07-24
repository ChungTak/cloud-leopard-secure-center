//! Multi-factor authentication factor and recovery code primitives.

use base64ct::{Base64UrlUnpadded, Encoding};
use foundation::{PlatformError, RandomSource, TenantId, UserId, UtcTimestamp, uuid::Uuid};
use sha2::{Digest, Sha256};

use crate::totp;

/// Supported MFA factor types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MfaFactorType {
    /// Time-based one-time password.
    Totp,
}

/// A single stored recovery code.
#[derive(Debug, Clone)]
pub struct RecoveryCode {
    /// Hash of the raw code, used for lookup and verification.
    pub hash: String,
    /// Whether this recovery code has already been consumed.
    pub used: bool,
}

impl RecoveryCode {
    /// Create a recovery code from a raw value. The raw value is not stored.
    pub fn from_raw(raw: &str) -> Self {
        Self {
            hash: hash_raw(raw),
            used: false,
        }
    }

    /// Check whether `raw` matches this recovery code without consuming it.
    pub fn matches(&self, raw: &str) -> bool {
        constant_time_eq(&hash_raw(raw), &self.hash)
    }
}

/// A user's MFA factor. The factor stores a reference to the secret, never the
/// secret itself. Recovery code hashes are stored for one-time use.
#[derive(Debug, Clone)]
pub struct MfaFactor {
    pub id: Uuid,
    pub tenant_id: TenantId,
    pub user_id: UserId,
    pub factor_type: MfaFactorType,
    /// External reference to the actual TOTP secret (e.g. a secret manager name).
    pub secret_ref: String,
    /// Whether this factor is currently active.
    pub enabled: bool,
    /// When the factor was verified during enrollment.
    pub verified_at: Option<UtcTimestamp>,
    pub created_at: UtcTimestamp,
    /// Recovery code hashes; raw codes are never persisted.
    pub recovery_codes: Vec<RecoveryCode>,
    /// Last accepted TOTP step, used to detect code replay.
    pub last_used_step: Option<u64>,
    /// Last accepted TOTP code, used with `last_used_step` for replay detection.
    pub last_used_code: Option<String>,
}

impl MfaFactor {
    /// Create and return a new TOTP factor with generated recovery codes.
    ///
    /// Returns the factor plus the raw recovery codes (one-time display only).
    pub fn new_totp(
        id: Uuid,
        tenant_id: TenantId,
        user_id: UserId,
        secret_ref: impl Into<String>,
        recovery_code_count: usize,
        random: &dyn RandomSource,
        clock: &dyn foundation::Clock,
    ) -> Result<(Self, Vec<String>), PlatformError> {
        const MAX_RECOVERY_CODES: usize = 32;
        if recovery_code_count > MAX_RECOVERY_CODES {
            return Err(PlatformError::invalid(
                "recovery_code_count",
                format!("must be at most {MAX_RECOVERY_CODES}"),
            ));
        }
        let mut raw_codes = Vec::with_capacity(recovery_code_count);
        let mut recovery_codes = Vec::with_capacity(recovery_code_count);
        for _ in 0..recovery_code_count {
            let mut bytes = [0u8; 8];
            random.fill_bytes(&mut bytes)?;
            let raw = Base64UrlUnpadded::encode_string(&bytes);
            recovery_codes.push(RecoveryCode::from_raw(&raw));
            raw_codes.push(raw);
        }

        let secret_ref = secret_ref.into();
        validate_secret_ref(&secret_ref)?;
        let factor = Self {
            id,
            tenant_id,
            user_id,
            factor_type: MfaFactorType::Totp,
            secret_ref,
            enabled: true,
            verified_at: Some(clock.now()),
            created_at: clock.now(),
            recovery_codes,
            last_used_step: None,
            last_used_code: None,
        };
        Ok((factor, raw_codes))
    }

    /// Verify a TOTP code for this factor, using the secret resolved from `secret_ref`.
    /// Updates internal replay-prevention state on success.
    pub fn verify_totp(
        &mut self,
        secret: &[u8],
        code: &str,
        now: UtcTimestamp,
    ) -> Result<bool, PlatformError> {
        if !self.enabled {
            return Ok(false);
        }
        let step = time_step(now);
        if Some(step) == self.last_used_step && Some(code.to_string()) == self.last_used_code {
            return Ok(false);
        }
        if let Some(matched_step) = totp::verify(secret, code, now)? {
            self.last_used_step = Some(matched_step);
            self.last_used_code = Some(code.to_string());
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Consume a recovery code. Returns true if a matching unused code was found.
    pub fn use_recovery_code(&mut self, raw: &str) -> bool {
        for code in &mut self.recovery_codes {
            if !code.used && code.matches(raw) {
                code.used = true;
                return true;
            }
        }
        false
    }

    /// Return the recovery code hashes, e.g. for persistence.
    pub fn recovery_code_hashes(&self) -> Vec<String> {
        self.recovery_codes.iter().map(|c| c.hash.clone()).collect()
    }

    /// Return the used flags aligned with `recovery_code_hashes`.
    pub fn recovery_code_used(&self) -> Vec<bool> {
        self.recovery_codes.iter().map(|c| c.used).collect()
    }

    /// Build an `MfaFactor` from persisted fields.
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        id: Uuid,
        tenant_id: TenantId,
        user_id: UserId,
        factor_type: MfaFactorType,
        secret_ref: String,
        enabled: bool,
        verified_at: Option<UtcTimestamp>,
        created_at: UtcTimestamp,
        recovery_code_hashes: Vec<String>,
        recovery_code_used: Vec<bool>,
        last_used_step: Option<u64>,
        last_used_code: Option<String>,
    ) -> Result<Self, PlatformError> {
        validate_secret_ref(&secret_ref)?;
        if recovery_code_hashes.len() != recovery_code_used.len() {
            return Err(PlatformError::invalid(
                "recovery_codes",
                "recovery code hashes and used flags must have the same length",
            ));
        }
        for hash in &recovery_code_hashes {
            validate_recovery_code_hash(hash)?;
        }
        match (last_used_step, &last_used_code) {
            (Some(_), None) | (None, Some(_)) => {
                return Err(PlatformError::invalid(
                    "last_used_step",
                    "last_used_step and last_used_code must both be present or both absent",
                ));
            }
            (Some(_), Some(code)) if code.trim().is_empty() => {
                return Err(PlatformError::invalid(
                    "last_used_code",
                    "last used code must not be empty when a step is present",
                ));
            }
            _ => {}
        }
        let recovery_codes = recovery_code_hashes
            .into_iter()
            .zip(recovery_code_used)
            .map(|(hash, used)| RecoveryCode { hash, used })
            .collect();
        Ok(Self {
            id,
            tenant_id,
            user_id,
            factor_type,
            secret_ref,
            enabled,
            verified_at,
            created_at,
            recovery_codes,
            last_used_step,
            last_used_code,
        })
    }
}

fn validate_secret_ref(secret_ref: &str) -> Result<(), PlatformError> {
    if secret_ref.trim().is_empty() {
        return Err(PlatformError::invalid(
            "secret_ref",
            "secret reference must not be empty",
        ));
    }
    if secret_ref.len() > 256 {
        return Err(PlatformError::invalid(
            "secret_ref",
            "secret reference must be at most 256 characters",
        ));
    }
    Ok(())
}

fn validate_recovery_code_hash(hash: &str) -> Result<(), PlatformError> {
    if hash.trim().is_empty() {
        return Err(PlatformError::invalid(
            "recovery_code_hash",
            "recovery code hash must not be empty",
        ));
    }
    Ok(())
}

fn hash_raw(raw: &str) -> String {
    let digest = Sha256::digest(raw.as_bytes());
    Base64UrlUnpadded::encode_string(&digest)
}

fn time_step(now: UtcTimestamp) -> u64 {
    let seconds = (now.timestamp_millis() / 1000) as u64;
    seconds / 30
}

fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.bytes()
        .zip(b.bytes())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}
