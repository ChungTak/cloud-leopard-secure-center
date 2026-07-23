//! User aggregate.

use foundation::{Clock, PlatformError, Revision, TenantId, UserId, UtcTimestamp};

/// Lifecycle status of a user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserStatus {
    /// Newly created; not yet activated.
    Pending,
    /// Active and allowed to authenticate.
    Active,
    /// Temporarily locked due to policy or admin action.
    Locked,
    /// Disabled and cannot be used.
    Disabled,
}

impl UserStatus {
    /// Parse a status string into the typed enum.
    pub fn parse(input: &str) -> Result<Self, PlatformError> {
        match input {
            "pending" => Ok(Self::Pending),
            "active" => Ok(Self::Active),
            "locked" => Ok(Self::Locked),
            "disabled" => Ok(Self::Disabled),
            _ => Err(PlatformError::invalid(
                "user_status",
                format!("unsupported status: {input}"),
            )),
        }
    }

    /// Return the canonical database representation.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Active => "active",
            Self::Locked => "locked",
            Self::Disabled => "disabled",
        }
    }
}

/// A domain event emitted by a `User` state change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserDomainEvent {
    /// User was created.
    Created {
        user_id: UserId,
        tenant_id: TenantId,
    },
    /// User was activated.
    Activated {
        user_id: UserId,
        tenant_id: TenantId,
    },
    /// User was locked.
    Locked {
        user_id: UserId,
        tenant_id: TenantId,
    },
    /// User was disabled.
    Disabled {
        user_id: UserId,
        tenant_id: TenantId,
    },
    /// Username was changed.
    UsernameChanged {
        user_id: UserId,
        tenant_id: TenantId,
        new_username: String,
    },
    /// Session version was bumped.
    SessionVersionBumped {
        user_id: UserId,
        tenant_id: TenantId,
        version: u64,
    },
    /// Disabled user was re-enabled.
    ReEnabled {
        user_id: UserId,
        tenant_id: TenantId,
    },
}

/// Normalizes a username: lowercase, trimmed, with only ASCII alphanumeric,
/// dots, dashes and underscores.
pub fn normalize_username(input: &str) -> Result<String, PlatformError> {
    let trimmed = input.trim().to_lowercase();
    if trimmed.is_empty() || trimmed.len() > 128 {
        return Err(PlatformError::invalid(
            "username",
            "username must be between 1 and 128 characters",
        ));
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
    {
        return Err(PlatformError::invalid(
            "username",
            "username contains invalid characters",
        ));
    }
    Ok(trimmed)
}

/// A user account.
#[derive(Debug, Clone)]
pub struct User {
    /// Unique user identifier.
    pub id: UserId,
    /// Owning tenant.
    pub tenant_id: TenantId,
    /// Normalized username.
    pub username: String,
    /// Display name.
    pub display_name: String,
    /// Current status.
    pub status: UserStatus,
    /// Ever-increasing session version used to revoke sessions.
    pub session_version: u64,
    /// Optimistic lock version.
    pub revision: Revision,
    /// Creation timestamp.
    pub created_at: UtcTimestamp,
    /// Last update timestamp.
    pub updated_at: UtcTimestamp,
    /// Actor that performed the last change.
    pub actor: Option<UserId>,
    /// Soft-delete timestamp.
    pub deleted_at: Option<UtcTimestamp>,
    /// Domain events produced by this aggregate that are not yet published.
    pub pending_events: Vec<UserDomainEvent>,
}

impl User {
    /// Create a new pending user.
    pub fn new(
        id: UserId,
        tenant_id: TenantId,
        username: impl Into<String>,
        display_name: impl Into<String>,
        clock: &dyn Clock,
        actor: Option<UserId>,
    ) -> Result<Self, PlatformError> {
        let username = normalize_username(&username.into())?;
        let display_name = display_name.into().trim().to_string();
        if display_name.is_empty() {
            return Err(PlatformError::invalid(
                "display_name",
                "display name cannot be empty",
            ));
        }
        let now = clock.now();
        let pending_events = vec![UserDomainEvent::Created {
            user_id: id,
            tenant_id,
        }];

        Ok(Self {
            id,
            tenant_id,
            username,
            display_name,
            status: UserStatus::Pending,
            session_version: 1,
            revision: Revision::initial(),
            created_at: now,
            updated_at: now,
            actor,
            deleted_at: None,
            pending_events,
        })
    }

    /// Change the username and bump revision.
    pub fn set_username(
        &mut self,
        username: impl Into<String>,
        clock: &dyn Clock,
        actor: Option<UserId>,
    ) -> Result<(), PlatformError> {
        let username = normalize_username(&username.into())?;
        if username == self.username {
            return Ok(());
        }
        self.username = username.clone();
        self.bump(clock, actor);
        self.pending_events.push(UserDomainEvent::UsernameChanged {
            user_id: self.id,
            tenant_id: self.tenant_id,
            new_username: username,
        });
        Ok(())
    }

    /// Change display name.
    pub fn set_display_name(
        &mut self,
        display_name: impl Into<String>,
        clock: &dyn Clock,
        actor: Option<UserId>,
    ) -> Result<(), PlatformError> {
        let display_name = display_name.into().trim().to_string();
        if display_name.is_empty() {
            return Err(PlatformError::invalid(
                "display_name",
                "display name cannot be empty",
            ));
        }
        if display_name == self.display_name {
            return Ok(());
        }
        self.display_name = display_name;
        self.bump(clock, actor);
        Ok(())
    }

    /// Activate the user.
    pub fn activate(
        &mut self,
        clock: &dyn Clock,
        actor: Option<UserId>,
    ) -> Result<(), PlatformError> {
        match self.status {
            UserStatus::Pending | UserStatus::Locked => {
                self.transition_to(UserStatus::Active, clock, actor)?;
                Ok(())
            }
            UserStatus::Disabled => Err(PlatformError::invalid(
                "status",
                "disabled user must be re-enabled first",
            )),
            UserStatus::Active => Ok(()),
        }
    }

    /// Lock the user.
    pub fn lock(&mut self, clock: &dyn Clock, actor: Option<UserId>) -> Result<(), PlatformError> {
        if self.status == UserStatus::Disabled {
            return Err(PlatformError::invalid(
                "status",
                "disabled user cannot be locked",
            ));
        }
        self.transition_to(UserStatus::Locked, clock, actor)
    }

    /// Disable the user.
    pub fn disable(
        &mut self,
        clock: &dyn Clock,
        actor: Option<UserId>,
    ) -> Result<(), PlatformError> {
        self.transition_to(UserStatus::Disabled, clock, actor)
    }

    /// Re-enable a disabled user, setting status back to pending.
    pub fn enable(
        &mut self,
        clock: &dyn Clock,
        actor: Option<UserId>,
    ) -> Result<(), PlatformError> {
        if self.status != UserStatus::Disabled {
            return Ok(());
        }
        self.transition_to(UserStatus::Pending, clock, actor)
    }

    /// Bump the session version to invalidate existing sessions.
    pub fn bump_session_version(
        &mut self,
        clock: &dyn Clock,
        actor: Option<UserId>,
    ) -> Result<(), PlatformError> {
        if self.deleted_at.is_some() {
            return Err(PlatformError::invalid("user", "user is deleted"));
        }
        self.session_version += 1;
        self.bump(clock, actor);
        self.pending_events
            .push(UserDomainEvent::SessionVersionBumped {
                user_id: self.id,
                tenant_id: self.tenant_id,
                version: self.session_version,
            });
        Ok(())
    }

    /// Soft-delete the user.
    pub fn delete(&mut self, clock: &dyn Clock, actor: Option<UserId>) {
        if self.deleted_at.is_some() {
            return;
        }
        self.deleted_at = Some(clock.now());
        self.bump(clock, actor);
    }

    fn transition_to(
        &mut self,
        to: UserStatus,
        clock: &dyn Clock,
        actor: Option<UserId>,
    ) -> Result<(), PlatformError> {
        if self.deleted_at.is_some() {
            return Err(PlatformError::invalid("user", "user is deleted"));
        }
        if self.status == to {
            return Ok(());
        }
        self.status = to;
        self.bump(clock, actor);
        let event = match to {
            UserStatus::Active => UserDomainEvent::Activated {
                user_id: self.id,
                tenant_id: self.tenant_id,
            },
            UserStatus::Locked => UserDomainEvent::Locked {
                user_id: self.id,
                tenant_id: self.tenant_id,
            },
            UserStatus::Disabled => UserDomainEvent::Disabled {
                user_id: self.id,
                tenant_id: self.tenant_id,
            },
            UserStatus::Pending => UserDomainEvent::ReEnabled {
                user_id: self.id,
                tenant_id: self.tenant_id,
            },
        };
        self.pending_events.push(event);
        Ok(())
    }

    fn bump(&mut self, clock: &dyn Clock, actor: Option<UserId>) {
        self.updated_at = clock.now();
        self.actor = actor;
        self.revision = self.revision.next();
    }
}
