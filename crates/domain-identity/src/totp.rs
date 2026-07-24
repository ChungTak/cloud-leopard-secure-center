//! TOTP (RFC 6238) verification using HMAC-SHA1.

use foundation::{PlatformError, UtcTimestamp};
use hmac::{Hmac, KeyInit, Mac};
use sha1::Sha1;
use subtle::ConstantTimeEq;

type HmacSha1 = Hmac<Sha1>;

const TIME_STEP_SECONDS: u64 = 30;
const CODE_DIGITS: u32 = 6;

/// Return the current TOTP code for `secret` at `now`.
pub fn current_code(secret: &[u8], now: UtcTimestamp) -> Result<String, PlatformError> {
    code_for_step(secret, time_step(now)?)
}

/// Return the TOTP code for `secret` at the given time `step`.
fn code_for_step(secret: &[u8], step: u64) -> Result<String, PlatformError> {
    let msg = step.to_be_bytes();
    let mut mac = HmacSha1::new_from_slice(secret)
        .map_err(|_| PlatformError::invalid("totp", "invalid secret"))?;
    mac.update(&msg);
    let result = mac.finalize().into_bytes();
    let bytes: &[u8] = result.as_ref();

    let offset = (bytes[bytes.len() - 1] & 0x0f) as usize;
    let code = ((u32::from(bytes[offset]) & 0x7f) << 24)
        | (u32::from(bytes[offset + 1]) << 16)
        | (u32::from(bytes[offset + 2]) << 8)
        | u32::from(bytes[offset + 3]);
    let code = code % 10u32.pow(CODE_DIGITS);
    Ok(format!("{:0digits$}", code, digits = CODE_DIGITS as usize))
}

/// Verify a user-supplied TOTP `code` against `secret` at `now`.
///
/// Returns the matched time step on success, or `None` if the code is invalid.
/// The verifier accepts the current step and one step on either side to handle
/// clock skew and boundary transitions.
pub fn verify(secret: &[u8], code: &str, now: UtcTimestamp) -> Result<Option<u64>, PlatformError> {
    let step = i64::try_from(time_step(now)?)
        .map_err(|_| PlatformError::invalid("totp", "time step is out of the supported range"))?;
    for candidate in [step - 1, step, step + 1] {
        if candidate < 0 {
            continue;
        }
        let candidate = candidate as u64;
        let expected = code_for_step(secret, candidate)?;
        if expected.as_bytes().ct_eq(code.as_bytes()).into() {
            return Ok(Some(candidate));
        }
    }
    Ok(None)
}

fn time_step(now: UtcTimestamp) -> Result<u64, PlatformError> {
    let millis: u64 = now
        .timestamp_millis()
        .try_into()
        .map_err(|_| PlatformError::invalid("totp", "timestamp is out of the supported range"))?;
    Ok(millis / 1000 / TIME_STEP_SECONDS)
}
