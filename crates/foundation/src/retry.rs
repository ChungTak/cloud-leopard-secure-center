//! Deterministic retry scheduling helpers.

use crate::{RandomSource, UtcTimestamp};
use chrono::Duration;

/// Describes how a transient failure should be retried.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    base_delay: Duration,
    max_delay: Duration,
    jitter: Duration,
    max_attempts: Option<u32>,
    deadline: Option<UtcTimestamp>,
}

impl RetryPolicy {
    /// Create a policy with exponential backoff and bounded jitter.
    ///
    /// `base_ms` and `max_ms` are clamped to `i64::MAX` milliseconds before
    /// being converted to `chrono::Duration`.
    pub fn exponential(
        base_ms: u64,
        max_ms: u64,
        jitter_ms: u64,
        max_attempts: Option<u32>,
        deadline: Option<UtcTimestamp>,
    ) -> Self {
        let as_duration = |ms: u64| -> Duration {
            let clamped = if ms > i64::MAX as u64 {
                i64::MAX
            } else {
                ms as i64
            };
            Duration::milliseconds(clamped)
        };

        Self {
            base_delay: as_duration(base_ms),
            max_delay: as_duration(max_ms),
            jitter: as_duration(jitter_ms),
            max_attempts,
            deadline,
        }
    }

    /// Compute the next scheduled retry time, or `None` if the job should not
    /// be retried (deadline exceeded, max attempts reached, or jitter overflow).
    pub fn next_retry(
        &self,
        now: UtcTimestamp,
        attempt: u32,
        random: &dyn RandomSource,
    ) -> Option<UtcTimestamp> {
        if self.max_attempts.is_some_and(|max| attempt >= max) {
            return None;
        }

        if self.deadline.is_some_and(|deadline| now > deadline) {
            return None;
        }

        let max_ms = self.max_delay.num_milliseconds().max(0) as u64;
        let delay_ms = exponential_delay_ms(
            self.base_delay.num_milliseconds().max(0) as u64,
            max_ms,
            attempt,
        );
        let jitter_ms = if self.jitter.num_milliseconds() > 0 {
            random_jitter_ms(self.jitter.num_milliseconds() as u64, random)
        } else {
            0
        };

        let total_ms = delay_ms.saturating_add(jitter_ms).min(max_ms);
        let next: chrono::DateTime<chrono::Utc> = now.into();
        let next = next.checked_add_signed(Duration::milliseconds(total_ms as i64))?;

        if self
            .deadline
            .is_some_and(|deadline| UtcTimestamp::from(next) > deadline)
        {
            return None;
        }

        Some(UtcTimestamp::from(next))
    }
}

fn exponential_delay_ms(base_ms: u64, max_ms: u64, attempt: u32) -> u64 {
    if attempt == 0 {
        return base_ms.min(max_ms);
    }
    let shift = (attempt.saturating_sub(1)).min(63);
    let multiplier = 1u64.checked_shl(shift).unwrap_or(u64::MAX);
    base_ms.saturating_mul(multiplier).min(max_ms)
}

fn random_jitter_ms(max_jitter_ms: u64, random: &dyn RandomSource) -> u64 {
    if max_jitter_ms == 0 {
        return 0;
    }
    let mut buf = [0u8; 8];
    if random.fill_bytes(&mut buf).is_err() {
        return 0;
    }
    let raw = u64::from_le_bytes(buf);
    raw % (max_jitter_ms + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct ZeroRandom;

    impl RandomSource for ZeroRandom {
        fn fill_bytes(&self, buf: &mut [u8]) -> Result<(), crate::PlatformError> {
            for b in buf.iter_mut() {
                *b = 0;
            }
            Ok(())
        }
    }

    #[derive(Debug)]
    struct MaxRandom;

    impl RandomSource for MaxRandom {
        fn fill_bytes(&self, buf: &mut [u8]) -> Result<(), crate::PlatformError> {
            for b in buf.iter_mut() {
                *b = 0xff;
            }
            Ok(())
        }
    }

    fn ts(millis: i64) -> UtcTimestamp {
        chrono::DateTime::from_timestamp_millis(millis)
            .map(UtcTimestamp::from)
            .unwrap_or_else(|| UtcTimestamp::from(chrono::DateTime::<chrono::Utc>::MIN_UTC))
    }

    #[test]
    fn exponential_delay_doubles_and_caps() {
        assert_eq!(exponential_delay_ms(100, 1000, 0), 100);
        assert_eq!(exponential_delay_ms(100, 1000, 1), 100);
        assert_eq!(exponential_delay_ms(100, 1000, 2), 200);
        assert_eq!(exponential_delay_ms(100, 1000, 3), 400);
        assert_eq!(exponential_delay_ms(100, 1000, 4), 800);
        assert_eq!(exponential_delay_ms(100, 1000, 5), 1000);
        assert_eq!(exponential_delay_ms(100, 1000, 10), 1000);
    }

    #[test]
    fn retry_respects_max_attempts_and_deadline() {
        let policy = RetryPolicy::exponential(100, 1000, 0, Some(2), None);
        let now = ts(0);
        assert!(policy.next_retry(now, 1, &ZeroRandom).is_some());
        assert!(policy.next_retry(now, 2, &ZeroRandom).is_none());

        let deadline = ts(50);
        let policy = RetryPolicy::exponential(100, 1000, 0, None, Some(deadline));
        assert!(policy.next_retry(ts(0), 1, &ZeroRandom).is_none());
    }

    #[test]
    fn retry_adds_jitter_and_caps() {
        let deadline = ts(10_000);
        let policy = RetryPolicy::exponential(100, 500, 50, None, Some(deadline));
        let Some(zero) = policy.next_retry(ts(0), 1, &ZeroRandom) else {
            panic!("expected retry");
        };
        let Some(max) = policy.next_retry(ts(0), 1, &MaxRandom) else {
            panic!("expected retry");
        };
        assert!(max.timestamp_millis() - zero.timestamp_millis() <= 50);
    }
}
