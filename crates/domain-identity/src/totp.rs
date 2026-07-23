//! TOTP (RFC 6238) verification using HMAC-SHA1.

use foundation::{PlatformError, UtcTimestamp};
use hmac::{Hmac, KeyInit, Mac};
use sha1::Sha1;

type HmacSha1 = Hmac<Sha1>;

const TIME_STEP_SECONDS: u64 = 30;
const CODE_DIGITS: u32 = 6;

/// Return the current TOTP code for `secret` at `now`.
pub fn current_code(secret: &[u8], now: UtcTimestamp) -> Result<String, PlatformError> {
    let step = time_step(now);
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
pub fn verify(secret: &[u8], code: &str, now: UtcTimestamp) -> Result<bool, PlatformError> {
    let expected = current_code(secret, now)?;
    Ok(constant_time_eq(&expected, code))
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

fn time_step(now: UtcTimestamp) -> u64 {
    let seconds = (now.timestamp_millis() / 1000) as u64;
    seconds / TIME_STEP_SECONDS
}
