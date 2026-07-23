//! MFA (TOTP) and assurance use cases.

use base64ct::{Base64UrlUnpadded, Encoding};
use domain_identity::assurance::AssuranceLevel;
use domain_identity::mfa::MfaFactor;
use foundation::{Clock, ErrorCode, PlatformError, RandomSource, RequestContext, UserId};
use storage_api::MfaRepository;

/// Resolves an MFA secret reference to the actual secret bytes.
/// Production implementations fetch from a secrets manager; tests can use an
/// in-memory map.
pub trait SecretResolver: Send + Sync {
    /// Store a secret under `ref_name`.
    fn store(&self, ref_name: &str, value: &[u8]) -> Result<(), PlatformError>;
    /// Resolve `ref_name` to secret bytes, if known.
    fn resolve(&self, ref_name: &str) -> Result<Option<Vec<u8>>, PlatformError>;
}

/// Result of enrolling a TOTP factor.
#[derive(Debug, Clone)]
pub struct EnrolledTotp {
    /// Persisted factor (stores only the secret reference and recovery hashes).
    pub factor: MfaFactor,
    /// Raw TOTP secret bytes, displayed once to the user.
    pub raw_secret: Vec<u8>,
    /// Raw recovery codes, displayed once to the user.
    pub recovery_codes: Vec<String>,
}

/// Enroll a new TOTP factor for a user.
#[allow(clippy::too_many_arguments)]
pub async fn enroll_totp(
    repo: &dyn MfaRepository,
    resolver: &dyn SecretResolver,
    random: &dyn RandomSource,
    clock: &dyn Clock,
    ctx: &RequestContext,
    user_id: UserId,
    recovery_code_count: usize,
) -> Result<EnrolledTotp, PlatformError> {
    let tenant_id = ctx.tenant_id.ok_or(PlatformError::new(
        ErrorCode::Unauthenticated,
        "missing tenant",
    ))?;

    let mut secret = vec![0u8; 32];
    random.fill_bytes(&mut secret)?;

    let mut ref_bytes = [0u8; 16];
    random.fill_bytes(&mut ref_bytes)?;
    let secret_ref = Base64UrlUnpadded::encode_string(&ref_bytes);

    resolver.store(&secret_ref, &secret)?;

    let id = foundation::generate_uuid(clock, random)?;

    let (factor, recovery_codes) = MfaFactor::new_totp(
        id,
        tenant_id,
        user_id,
        &secret_ref,
        recovery_code_count,
        random,
        clock,
    )?;
    repo.save_factor(&factor, ctx).await?;

    Ok(EnrolledTotp {
        factor,
        raw_secret: secret,
        recovery_codes,
    })
}

/// Verify a TOTP code for `user_id`. Updates replay-prevention state on success.
pub async fn verify_totp(
    repo: &dyn MfaRepository,
    resolver: &dyn SecretResolver,
    clock: &dyn Clock,
    ctx: &RequestContext,
    user_id: UserId,
    code: &str,
) -> Result<(), PlatformError> {
    let mut factor = match repo.find_active_factor_by_user(user_id, ctx).await? {
        Some(f) => f,
        None => {
            return Err(PlatformError::new(
                ErrorCode::Unauthenticated,
                "mfa required",
            ));
        }
    };

    let secret = resolver
        .resolve(&factor.secret_ref)?
        .ok_or_else(|| PlatformError::new(ErrorCode::Unavailable, "mfa secret not found"))?;

    if factor.verify_totp(&secret, code, clock.now())? {
        repo.update_factor(&factor, ctx).await?;
        Ok(())
    } else {
        Err(PlatformError::new(
            ErrorCode::Unauthenticated,
            "invalid mfa code",
        ))
    }
}

/// Consume a recovery code for `user_id`.
pub async fn use_recovery_code(
    repo: &dyn MfaRepository,
    ctx: &RequestContext,
    user_id: UserId,
    code: &str,
) -> Result<(), PlatformError> {
    let mut factor = match repo.find_active_factor_by_user(user_id, ctx).await? {
        Some(f) => f,
        None => {
            return Err(PlatformError::new(
                ErrorCode::Unauthenticated,
                "mfa required",
            ));
        }
    };

    if factor.use_recovery_code(code) {
        repo.update_factor(&factor, ctx).await?;
        Ok(())
    } else {
        Err(PlatformError::new(
            ErrorCode::Unauthenticated,
            "invalid recovery code",
        ))
    }
}

/// Ensure `current` assurance meets `required`.
pub fn require_assurance(
    current: AssuranceLevel,
    required: AssuranceLevel,
) -> Result<(), PlatformError> {
    if current.meets(required) {
        Ok(())
    } else {
        Err(PlatformError::new(
            ErrorCode::Denied,
            "insufficient assurance level",
        ))
    }
}
