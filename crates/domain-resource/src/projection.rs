//! Signaling projection read models and events.

use foundation::{PlatformError, UtcTimestamp};

/// An event describing the observed state of a device from an external source.
#[derive(Debug, Clone)]
pub struct DeviceEvent {
    pub external_ref: String,
    pub sequence: i64,
    pub source_event_id: String,
    pub observed_at: UtcTimestamp,
    pub payload: String,
}

/// An event describing the observed state of a channel from an external source.
#[derive(Debug, Clone)]
pub struct ChannelEvent {
    pub external_ref: String,
    pub sequence: i64,
    pub source_event_id: String,
    pub observed_at: UtcTimestamp,
    pub payload: String,
}

/// Projected state of a device returned by the API.
#[derive(Debug, Clone)]
pub struct DeviceProjection {
    pub external_ref: String,
    pub sequence: i64,
    pub source_event_id: String,
    pub observed_at: UtcTimestamp,
    pub payload: String,
    pub stale: bool,
}

/// Projected state of a channel returned by the API.
#[derive(Debug, Clone)]
pub struct ChannelProjection {
    pub external_ref: String,
    pub sequence: i64,
    pub source_event_id: String,
    pub observed_at: UtcTimestamp,
    pub payload: String,
    pub stale: bool,
}

/// A projection processing failure that preserves the original event for review.
#[derive(Debug, Clone)]
pub struct ProjectionFailure {
    pub id: String,
    pub source_event_id: String,
    pub external_ref: String,
    pub reason: String,
    pub payload: String,
}

impl DeviceEvent {
    /// Validate event fields.
    pub fn validate(&self) -> Result<(), PlatformError> {
        if self.external_ref.trim().is_empty() {
            return Err(PlatformError::invalid(
                "external_ref",
                "external_ref must not be empty",
            ));
        }
        if self.source_event_id.trim().is_empty() {
            return Err(PlatformError::invalid(
                "source_event_id",
                "source_event_id must not be empty",
            ));
        }
        Ok(())
    }

    /// Determine whether applying this event to a projection would leave the
    /// projection stale because of a sequence gap.
    pub fn is_contiguous(&self, last_sequence: Option<i64>) -> bool {
        match last_sequence {
            None => true,
            Some(last) => self.sequence == last + 1,
        }
    }
}

impl ChannelEvent {
    /// Validate event fields.
    pub fn validate(&self) -> Result<(), PlatformError> {
        if self.external_ref.trim().is_empty() {
            return Err(PlatformError::invalid(
                "external_ref",
                "external_ref must not be empty",
            ));
        }
        if self.source_event_id.trim().is_empty() {
            return Err(PlatformError::invalid(
                "source_event_id",
                "source_event_id must not be empty",
            ));
        }
        Ok(())
    }

    /// Determine whether applying this event to a projection would leave the
    /// projection stale because of a sequence gap.
    pub fn is_contiguous(&self, last_sequence: Option<i64>) -> bool {
        match last_sequence {
            None => true,
            Some(last) => self.sequence == last + 1,
        }
    }
}

/// Clock-based staleness check for a projection record.
pub fn is_projection_stale(observed_at: UtcTimestamp, now: UtcTimestamp, ttl_millis: i64) -> bool {
    now.timestamp_millis()
        .saturating_sub(observed_at.timestamp_millis())
        > ttl_millis
}

#[cfg(test)]
mod tests {
    use super::*;
    use foundation::{Clock, FakeClock};

    #[test]
    fn event_sequence_gaps_are_detected() {
        let clock = FakeClock::from_millis(1_000_000_000_000);
        let event = DeviceEvent {
            external_ref: "dev-1".to_string(),
            sequence: 5,
            source_event_id: "evt-5".to_string(),
            observed_at: clock.now(),
            payload: "{}".to_string(),
        };
        assert!(event.is_contiguous(Some(4)));
        assert!(!event.is_contiguous(Some(2)));
    }
}
