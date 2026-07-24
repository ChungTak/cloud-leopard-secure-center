//! Notification port and value objects.
//!
//! Phase 1 freezes the `NotificationPort` contract and the template variable
//! whitelist. Real delivery (in-app, SSE, webhook) is deferred and represented
//! by the `UnsupportedNotificationPort` stub.

use std::collections::{HashMap, HashSet};

use foundation::{Deadline, TenantId};

const MAX_RECIPIENT_LEN: usize = 1024;
const MAX_TEMPLATE_LEN: usize = 256;
const MAX_TEMPLATE_VARS: usize = 64;
const MAX_TEMPLATE_KEY_LEN: usize = 64;
const MAX_TEMPLATE_VALUE_LEN: usize = 4096;

/// Supported notification channels. New channels can be added without breaking
/// the port.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum NotificationChannel {
    InApp,
    Sse,
    Webhook,
}

/// A notification request.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Notification {
    pub tenant_id: TenantId,
    pub channel: NotificationChannel,
    pub recipient: String,
    pub template: String,
    pub template_vars: HashMap<String, String>,
    pub deadline: Option<Deadline>,
}

impl Notification {
    /// Validate the notification shape and bounds before delivery.
    pub fn validate(&self) -> Result<(), NotificationError> {
        if self.recipient.trim().is_empty() || self.recipient.len() > MAX_RECIPIENT_LEN {
            return Err(NotificationError::new(
                NotificationErrorKind::Invalid,
                "recipient is empty or exceeds maximum length",
            ));
        }
        if self.template.trim().is_empty() || self.template.len() > MAX_TEMPLATE_LEN {
            return Err(NotificationError::new(
                NotificationErrorKind::Invalid,
                "template is empty or exceeds maximum length",
            ));
        }
        if self.template_vars.len() > MAX_TEMPLATE_VARS {
            return Err(NotificationError::new(
                NotificationErrorKind::Invalid,
                "too many template variables",
            ));
        }
        for (key, value) in &self.template_vars {
            if key.trim().is_empty() || key.len() > MAX_TEMPLATE_KEY_LEN {
                return Err(NotificationError::new(
                    NotificationErrorKind::Invalid,
                    "template variable key is empty or too long",
                ));
            }
            if value.len() > MAX_TEMPLATE_VALUE_LEN {
                return Err(NotificationError::new(
                    NotificationErrorKind::Invalid,
                    "template variable value is too long",
                ));
            }
        }
        Ok(())
    }

    /// Validate that every template variable is in the whitelist.
    pub fn validate_vars(&self, whitelist: &HashSet<String>) -> Result<(), NotificationError> {
        for key in self.template_vars.keys() {
            if !whitelist.contains(key) {
                return Err(NotificationError::new(
                    NotificationErrorKind::Invalid,
                    format!("template variable '{key}' is not in the whitelist"),
                ));
            }
        }
        Ok(())
    }
}

/// Notification domain error.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
#[error("{kind:?}: {message}")]
pub struct NotificationError {
    pub kind: NotificationErrorKind,
    pub message: String,
}

impl NotificationError {
    pub fn new(kind: NotificationErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

/// Kinds of notification failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum NotificationErrorKind {
    Invalid,
    Unsupported,
    Unavailable,
    Backpressure,
    Timeout,
    Unauthorized,
}

/// Port for sending notifications.
#[async_trait::async_trait]
pub trait NotificationPort: Send + Sync {
    async fn send(&self, notification: &Notification) -> Result<(), NotificationError>;
}

/// Placeholder notification port.
#[derive(Debug, Clone, Default)]
pub struct UnsupportedNotificationPort {
    enabled: bool,
}

impl UnsupportedNotificationPort {
    /// Create a port. When `enabled` is true the port reports `Unsupported`;
    /// otherwise `Unavailable`.
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }
}

#[async_trait::async_trait]
impl NotificationPort for UnsupportedNotificationPort {
    async fn send(&self, notification: &Notification) -> Result<(), NotificationError> {
        notification.validate()?;
        if self.enabled {
            Err(NotificationError::new(
                NotificationErrorKind::Unsupported,
                "notification delivery is not implemented in this build",
            ))
        } else {
            Err(NotificationError::new(
                NotificationErrorKind::Unavailable,
                "notification channels are not configured",
            ))
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use foundation::{SystemClock, SystemIdGenerator, SystemRandom};

    use super::*;

    fn err_or_panic<T, E: std::fmt::Debug>(result: Result<T, E>) -> E {
        match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        }
    }

    fn make_notification() -> Notification {
        let mut vars = HashMap::new();
        vars.insert("alarm_title".to_string(), "motion".to_string());
        let generator = SystemIdGenerator::new(SystemClock, SystemRandom);
        Notification {
            tenant_id: TenantId::generate(&generator).expect("generate tenant id"),
            channel: NotificationChannel::Webhook,
            recipient: "https://example.com/hook".to_string(),
            template: "alarm".to_string(),
            template_vars: vars,
            deadline: None,
        }
    }

    #[test]
    fn valid_template_vars_pass_whitelist() {
        let n = make_notification();
        let whitelist: HashSet<String> = ["alarm_title".to_string()].into_iter().collect();
        assert!(n.validate_vars(&whitelist).is_ok());
    }

    #[test]
    fn unknown_template_var_fails_whitelist() {
        let mut n = make_notification();
        n.template_vars
            .insert("exploit".to_string(), "x".to_string());
        let whitelist: HashSet<String> = ["alarm_title".to_string()].into_iter().collect();
        let result = n.validate_vars(&whitelist);
        assert_eq!(err_or_panic(result).kind, NotificationErrorKind::Invalid);
    }

    #[test]
    fn disabled_port_returns_unavailable() {
        futures::executor::block_on(async {
            let port = UnsupportedNotificationPort::new(false);
            let result = port.send(&make_notification()).await;
            assert_eq!(
                err_or_panic(result).kind,
                NotificationErrorKind::Unavailable
            );
        });
    }

    #[test]
    fn enabled_port_returns_unsupported() {
        futures::executor::block_on(async {
            let port = UnsupportedNotificationPort::new(true);
            let result = port.send(&make_notification()).await;
            assert_eq!(
                err_or_panic(result).kind,
                NotificationErrorKind::Unsupported
            );
        });
    }
}
