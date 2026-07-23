//! Cloud Leopard Secure Center foundation types and utilities.

pub mod config;
pub mod retry;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::atomic::{AtomicU8, Ordering};
use uuid::Uuid;

pub use chrono;
pub use uuid;

/// Strongly typed identifier based on UUIDv7.
macro_rules! id_newtype {
    ($name:ident) => {
        /// A strongly typed UUID-based identifier.
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            /// Generate a new identifier using the provided generator.
            pub fn generate(generator: &dyn IdGenerator) -> Self {
                Self(generator.generate())
            }

            /// Parse a hyphenated UUID string into this identifier type.
            pub fn parse_str(input: &str) -> Result<Self, PlatformError> {
                Uuid::parse_str(input)
                    .map(Self)
                    .map_err(|e| PlatformError::invalid(stringify!($name), e.to_string()))
            }

            /// Access the underlying UUID.
            pub const fn as_uuid(&self) -> &Uuid {
                &self.0
            }

            /// Return the canonical hyphenated string form.
            pub fn to_hyphenated(&self) -> String {
                self.0.to_string()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl From<$name> for Uuid {
            fn from(value: $name) -> Self {
                value.0
            }
        }
    };
}

id_newtype!(TenantId);
id_newtype!(UserId);
id_newtype!(RoleId);
id_newtype!(OrganizationId);
id_newtype!(SiteId);
id_newtype!(BuildingId);
id_newtype!(FloorId);
id_newtype!(AreaId);
id_newtype!(DeviceId);
id_newtype!(CameraId);
id_newtype!(BindingId);
id_newtype!(AuditId);
id_newtype!(MessageId);
id_newtype!(AlarmId);
id_newtype!(NodeId);
id_newtype!(TagId);
id_newtype!(ExternalBindingId);
id_newtype!(OperationId);
id_newtype!(MediaSessionId);
id_newtype!(EntitlementId);

/// An opaque resource reference.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ResourceRef(String);

impl ResourceRef {
    /// Parse a non-empty resource reference string.
    pub fn parse(input: &str) -> Result<Self, PlatformError> {
        if input.is_empty() {
            return Err(PlatformError::invalid(
                "resource_ref",
                "resource reference must not be empty".to_string(),
            ));
        }
        Ok(Self(input.to_string()))
    }

    /// Borrow the inner string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ResourceRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A monotonically increasing revision counter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Revision(pub u64);

impl Revision {
    /// Create the initial revision for a new aggregate.
    pub const fn initial() -> Self {
        Self(1)
    }

    /// Create a new revision.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Return the next revision.
    pub const fn next(self) -> Self {
        Self(self.0 + 1)
    }

    /// Return the previous revision, saturating at 1.
    pub const fn prev(self) -> Self {
        Self(self.0.saturating_sub(1))
    }

    /// Access the underlying value.
    pub const fn value(&self) -> u64 {
        self.0
    }
}

/// UTC timestamp used throughout the platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct UtcTimestamp(DateTime<Utc>);

impl UtcTimestamp {
    /// Current UTC timestamp.
    pub fn now() -> Self {
        Self(Utc::now())
    }

    /// Parse an RFC 3339 timestamp.
    pub fn parse_rfc3339(input: &str) -> Result<Self, PlatformError> {
        DateTime::parse_from_rfc3339(input)
            .map(|dt| Self(dt.with_timezone(&Utc)))
            .map_err(|e| PlatformError::invalid("timestamp", e.to_string()))
    }

    /// Format as RFC 3339.
    pub fn to_rfc3339(&self) -> String {
        self.0.to_rfc3339()
    }

    /// Return the Unix timestamp in milliseconds.
    pub fn timestamp_millis(&self) -> i64 {
        self.0.timestamp_millis()
    }
}

impl From<DateTime<Utc>> for UtcTimestamp {
    fn from(value: DateTime<Utc>) -> Self {
        Self(value)
    }
}

impl From<UtcTimestamp> for DateTime<Utc> {
    fn from(value: UtcTimestamp) -> Self {
        value.0
    }
}

/// A deadline relative to the platform clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Deadline(UtcTimestamp);

impl Deadline {
    /// Create a deadline from a timestamp.
    pub fn new(timestamp: UtcTimestamp) -> Self {
        Self(timestamp)
    }

    /// Determine whether the deadline has expired according to the supplied clock.
    pub fn is_expired(&self, clock: &dyn Clock) -> bool {
        clock.now() > self.0
    }

    /// Access the inner timestamp.
    pub const fn timestamp(&self) -> UtcTimestamp {
        self.0
    }
}

/// Clock abstraction; domain code must never call the system clock directly.
pub trait Clock: Send + Sync {
    /// Current platform timestamp.
    fn now(&self) -> UtcTimestamp;
}

/// System clock.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> UtcTimestamp {
        UtcTimestamp(Utc::now())
    }
}

/// Deterministic fake clock for tests.
#[derive(Debug, Clone, Copy)]
pub struct FakeClock {
    millis: i64,
}

impl FakeClock {
    /// Create a fake clock from a Unix millisecond timestamp.
    pub const fn from_millis(millis: i64) -> Self {
        Self { millis }
    }

    /// Advance the clock by the given number of milliseconds.
    pub const fn advance(&mut self, millis: i64) {
        self.millis += millis;
    }
}

impl Clock for FakeClock {
    fn now(&self) -> UtcTimestamp {
        let Some(naive) = DateTime::from_timestamp_millis(self.millis) else {
            panic!("invalid fake timestamp");
        };
        UtcTimestamp(naive)
    }
}

/// Source of random bytes; domain code must never read `/dev/urandom` directly.
pub trait RandomSource: Send + Sync {
    /// Fill `buf` with random bytes.
    fn fill_bytes(&self, buf: &mut [u8]);
}

/// System random source.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemRandom;

impl RandomSource for SystemRandom {
    fn fill_bytes(&self, buf: &mut [u8]) {
        if let Err(e) = getrandom::fill(buf) {
            panic!("system random source failed: {}", e);
        }
    }
}

/// Deterministic fake random source for tests.
#[derive(Debug)]
pub struct FakeRandom {
    next: AtomicU8,
}

impl Default for FakeRandom {
    fn default() -> Self {
        Self::new(0)
    }
}

impl FakeRandom {
    /// Create a fake random source starting with the given byte.
    pub const fn new(start: u8) -> Self {
        Self {
            next: AtomicU8::new(start),
        }
    }
}

impl RandomSource for FakeRandom {
    fn fill_bytes(&self, buf: &mut [u8]) {
        for b in buf.iter_mut() {
            *b = self.next.fetch_add(1, Ordering::SeqCst);
        }
    }
}

/// Generates UUIDv7 identifiers.
pub trait IdGenerator: Send + Sync {
    /// Generate a new UUID.
    fn generate(&self) -> Uuid;
}

/// Standard UUIDv7 generator backed by an injected clock and random source.
pub struct StandardIdGenerator<C, R> {
    clock: C,
    random: R,
}

impl<C, R> StandardIdGenerator<C, R> {
    /// Create a new standard generator.
    pub const fn new(clock: C, random: R) -> Self {
        Self { clock, random }
    }
}

impl<C: Clock, R: RandomSource> IdGenerator for StandardIdGenerator<C, R> {
    fn generate(&self) -> Uuid {
        let ts = self.clock.now().timestamp_millis() as u64;
        let mut rand = [0u8; 10];
        self.random.fill_bytes(&mut rand);
        let mut bytes = [0u8; 16];
        bytes[0..6].copy_from_slice(&ts.to_be_bytes()[2..8]);
        bytes[6..8].copy_from_slice(&rand[0..2]);
        bytes[7] = (bytes[7] & 0x0F) | 0x70; // version 7
        bytes[8..16].copy_from_slice(&rand[2..10]);
        bytes[8] = (bytes[8] & 0x3F) | 0x80; // variant 10
        Uuid::from_bytes(bytes)
    }
}

/// System-backed identifier generator.
pub type SystemIdGenerator = StandardIdGenerator<SystemClock, SystemRandom>;

/// Opaque pagination cursor.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PageCursor(String);

impl PageCursor {
    /// Parse a non-empty page cursor.
    pub fn parse(input: &str) -> Result<Self, PlatformError> {
        if input.is_empty() {
            return Err(PlatformError::invalid(
                "page_cursor",
                "cursor must not be empty".to_string(),
            ));
        }
        Ok(Self(input.to_string()))
    }

    /// Borrow the inner opaque cursor string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Stable error classification exposed to clients.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    Invalid,
    Unauthenticated,
    Denied,
    NotFound,
    Exists,
    Conflict,
    RateLimit,
    Timeout,
    Cancelled,
    Unavailable,
    Unsupported,
    VersionMismatch,
    UnknownOutcome,
    Internal,
}

/// Platform-wide error type.
#[derive(Debug, thiserror::Error, Clone, Serialize)]
#[serde(tag = "code", content = "details")]
pub enum PlatformError {
    /// Input or state is invalid.
    #[error("invalid {field}: {message}")]
    #[serde(rename = "invalid")]
    Invalid {
        /// The field or context that is invalid.
        field: String,
        /// A safe, stable, non-secret message.
        message: String,
    },
    /// Caller is not authenticated.
    #[error("unauthenticated")]
    #[serde(rename = "unauthenticated")]
    Unauthenticated,
    /// Caller is not authorized.
    #[error("denied")]
    #[serde(rename = "denied")]
    Denied,
    /// Requested resource was not found.
    #[error("not_found")]
    #[serde(rename = "not_found")]
    NotFound,
    /// Resource already exists.
    #[error("exists")]
    #[serde(rename = "exists")]
    Exists,
    /// Resource is in a conflicting state.
    #[error("conflict")]
    #[serde(rename = "conflict")]
    Conflict,
    /// Rate limit exceeded.
    #[error("rate_limit")]
    #[serde(rename = "rate_limit")]
    RateLimit,
    /// Operation timed out.
    #[error("timeout")]
    #[serde(rename = "timeout")]
    Timeout,
    /// Operation was cancelled.
    #[error("cancelled")]
    #[serde(rename = "cancelled")]
    Cancelled,
    /// Service unavailable.
    #[error("unavailable")]
    #[serde(rename = "unavailable")]
    Unavailable,
    /// Capability is not implemented.
    #[error("unsupported")]
    #[serde(rename = "unsupported")]
    Unsupported,
    /// Version mismatch.
    #[error("version_mismatch")]
    #[serde(rename = "version_mismatch")]
    VersionMismatch,
    /// Outcome of the operation is unknown.
    #[error("unknown_outcome")]
    #[serde(rename = "unknown_outcome")]
    UnknownOutcome,
    /// Internal error; message is safe and generatoreric.
    #[error("internal")]
    #[serde(rename = "internal")]
    Internal,
}

impl PlatformError {
    /// Create an error from a stable code and a public-safe message.
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        match code {
            ErrorCode::Invalid => Self::Invalid {
                field: "unknown".to_string(),
                message: message.into(),
            },
            ErrorCode::Unauthenticated => Self::Unauthenticated,
            ErrorCode::Denied => Self::Denied,
            ErrorCode::NotFound => Self::NotFound,
            ErrorCode::Exists => Self::Exists,
            ErrorCode::Conflict => Self::Conflict,
            ErrorCode::RateLimit => Self::RateLimit,
            ErrorCode::Timeout => Self::Timeout,
            ErrorCode::Cancelled => Self::Cancelled,
            ErrorCode::Unavailable => Self::Unavailable,
            ErrorCode::Unsupported => Self::Unsupported,
            ErrorCode::VersionMismatch => Self::VersionMismatch,
            ErrorCode::UnknownOutcome => Self::UnknownOutcome,
            ErrorCode::Internal => Self::Internal,
        }
    }

    /// Create an invalid-field error.
    pub fn invalid(field: &str, message: impl Into<String>) -> Self {
        Self::Invalid {
            field: field.to_string(),
            message: message.into(),
        }
    }

    /// Stable error code for this error.
    pub const fn code(&self) -> ErrorCode {
        match self {
            Self::Invalid { .. } => ErrorCode::Invalid,
            Self::Unauthenticated => ErrorCode::Unauthenticated,
            Self::Denied => ErrorCode::Denied,
            Self::NotFound => ErrorCode::NotFound,
            Self::Exists => ErrorCode::Exists,
            Self::Conflict => ErrorCode::Conflict,
            Self::RateLimit => ErrorCode::RateLimit,
            Self::Timeout => ErrorCode::Timeout,
            Self::Cancelled => ErrorCode::Cancelled,
            Self::Unavailable => ErrorCode::Unavailable,
            Self::Unsupported => ErrorCode::Unsupported,
            Self::VersionMismatch => ErrorCode::VersionMismatch,
            Self::UnknownOutcome => ErrorCode::UnknownOutcome,
            Self::Internal => ErrorCode::Internal,
        }
    }

    /// Public-safe response: error code and a generatoreric message.
    pub fn public_message(&self) -> String {
        self.to_string()
    }
}

/// Request-scoped context; must not contain database connections or HTTP extractors.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RequestContext {
    /// Unique request identifier.
    pub request_id: Option<MessageId>,
    /// Correlation identifier for distributed tracing.
    pub correlation_id: Option<MessageId>,
    /// Trace identifier.
    pub trace_id: Option<String>,
    /// Authenticated actor, if any.
    pub actor_id: Option<UserId>,
    /// Tenant scope, if any.
    pub tenant_id: Option<TenantId>,
    /// Request deadline, if any.
    pub deadline: Option<Deadline>,
    /// Organization scope, if any.
    pub organization_id: Option<OrganizationId>,
}

impl RequestContext {
    /// Create a builder-style context with the given request id.
    pub fn with_request_id(mut self, id: MessageId) -> Self {
        self.request_id = Some(id);
        self
    }

    /// Set the actor.
    pub fn with_actor(mut self, id: UserId) -> Self {
        self.actor_id = Some(id);
        self
    }

    /// Set the tenant.
    pub fn with_tenant(mut self, id: TenantId) -> Self {
        self.tenant_id = Some(id);
        self
    }

    /// Set the deadline.
    pub fn with_deadline(mut self, deadline: Deadline) -> Self {
        self.deadline = Some(deadline);
        self
    }
}

/// Public crate version.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[allow(dead_code)]
fn touch_dependencies() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_non_empty() {
        assert!(!version().is_empty());
    }

    #[test]
    fn id_round_trip() -> Result<(), PlatformError> {
        let generator = SystemIdGenerator::new(SystemClock, SystemRandom);
        let id = TenantId::generate(&generator);
        let parsed = TenantId::parse_str(&id.to_hyphenated())?;
        assert_eq!(id, parsed);
        Ok(())
    }

    #[test]
    fn different_id_types_are_incompatible() {
        let generator = SystemIdGenerator::new(SystemClock, SystemRandom);
        let tenant = TenantId::generate(&generator);
        let user = UserId::generate(&generator);
        assert_ne!(Uuid::from(tenant), Uuid::from(user));
    }

    #[test]
    fn timestamp_rfc3339_round_trip() -> Result<(), PlatformError> {
        let ts = UtcTimestamp::parse_rfc3339("2026-07-22T15:00:00Z")?;
        assert_eq!(ts.to_rfc3339(), "2026-07-22T15:00:00+00:00");
        Ok(())
    }

    #[test]
    fn page_cursor_round_trip() -> Result<(), PlatformError> {
        let cursor = PageCursor::parse("opaque-token")?;
        assert_eq!(cursor.as_str(), "opaque-token");
        Ok(())
    }

    #[test]
    fn fake_clock_is_deterministic() {
        let clock = FakeClock::from_millis(1_720_000_000_000);
        assert_eq!(clock.now().timestamp_millis(), 1_720_000_000_000);
    }

    #[test]
    fn deadline_expires() {
        let mut clock = FakeClock::from_millis(1_000_000);
        let deadline = Deadline::new(clock.now());
        assert!(!deadline.is_expired(&clock));
        clock.advance(1);
        assert!(deadline.is_expired(&clock));
    }

    #[test]
    fn error_code_is_stable() {
        let err = PlatformError::invalid("tenant", "bad uuid");
        assert_eq!(err.code(), ErrorCode::Invalid);
        assert!(err.public_message().contains("tenant"));
    }

    #[test]
    fn serialization_does_not_include_source() -> Result<(), serde_json::Error> {
        let err = PlatformError::invalid("field", "msg");
        let json = serde_json::to_string(&err)?;
        assert!(json.contains("invalid"));
        assert!(!json.contains("source"));
        Ok(())
    }

    #[test]
    fn fake_random_is_deterministic() {
        let random = FakeRandom::new(0);
        let mut buf = [0u8; 4];
        random.fill_bytes(&mut buf);
        assert_eq!(buf, [0, 1, 2, 3]);
    }

    #[test]
    fn fake_id_generator_is_deterministic() {
        let clock = FakeClock::from_millis(1_720_000_000_000);
        let random = FakeRandom::new(1);
        let generator = StandardIdGenerator::new(clock, random);
        let first = generator.generate();
        let second = generator.generate();
        assert_ne!(first, second);
    }
}
