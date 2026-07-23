//! Application use-case template tests using fake ports.

use application::authorization::{
    AuthorizationPort, AuthorizationRequest, AuthorizationResponse, Decision, Reason,
};
use application::tenant::{CreateTenantRequest, TenantService, TenantUseCase};
use application::usecase::{IdempotencyContext, WriteRequest};
use application::user::{CreateUserRequest, UserService, UserUseCase};
use async_trait::async_trait;
use domain_audit::audit_record::{AuditRecord, AuditRecordId};
use domain_organization::tenant::Tenant;
use foundation::{
    Clock, Deadline, ErrorCode, FakeClock, FakeRandom, IdGenerator, MessageId, PlatformError,
    RequestContext, Revision, StandardIdGenerator, TenantId, UserId, UtcTimestamp,
};
use std::sync::{Arc, Mutex};
use storage_api::{AuditWriter, Page, TenantRepository, UserRepository};

fn fake_clock() -> FakeClock {
    FakeClock::from_millis(1_000_000_000_000)
}

fn id_gen(clock: FakeClock) -> impl IdGenerator {
    StandardIdGenerator::new(clock, FakeRandom::new(0))
}

fn actor() -> UserId {
    UserId::parse_str("018e0000-0000-0000-0000-000000000001").unwrap_or_else(|e| panic!("{e:?}"))
}

fn tenant_ctx(tenant_id: TenantId) -> RequestContext {
    RequestContext {
        actor_id: Some(actor()),
        tenant_id: Some(tenant_id),
        request_id: Some(
            MessageId::parse_str("00000000-0000-0000-0000-000000000001")
                .unwrap_or_else(|e| panic!("{e:?}")),
        ),
        correlation_id: None,
        trace_id: None,
        deadline: None,
        organization_id: None,
    }
}

fn platform_ctx() -> RequestContext {
    RequestContext {
        actor_id: Some(actor()),
        tenant_id: None,
        request_id: Some(
            MessageId::parse_str("00000000-0000-0000-0000-000000000001")
                .unwrap_or_else(|e| panic!("{e:?}")),
        ),
        correlation_id: None,
        trace_id: None,
        deadline: None,
        organization_id: None,
    }
}

#[derive(Debug, Clone)]
struct FakeTenantRepo {
    tenants: Arc<Mutex<Vec<Tenant>>>,
}

impl FakeTenantRepo {
    fn new() -> Self {
        Self {
            tenants: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

#[async_trait]
impl TenantRepository for FakeTenantRepo {
    async fn by_id(&self, id: TenantId, _ctx: &RequestContext) -> Result<Tenant, PlatformError> {
        let tenants = self.tenants.lock().unwrap_or_else(|e| panic!("{e:?}"));
        tenants
            .iter()
            .find(|t| t.id == id)
            .cloned()
            .ok_or_else(|| PlatformError::new(ErrorCode::NotFound, "tenant not found".to_string()))
    }

    async fn create(&self, tenant: &Tenant, _ctx: &RequestContext) -> Result<(), PlatformError> {
        let mut tenants = self.tenants.lock().unwrap_or_else(|e| panic!("{e:?}"));
        if tenants.iter().any(|t| t.id == tenant.id) {
            return Err(PlatformError::new(
                ErrorCode::Conflict,
                "tenant already exists".to_string(),
            ));
        }
        tenants.push(tenant.clone());
        Ok(())
    }

    async fn update(
        &self,
        tenant: &Tenant,
        expected: Revision,
        _ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        let mut tenants = self.tenants.lock().unwrap_or_else(|e| panic!("{e:?}"));
        let existing = tenants
            .iter_mut()
            .find(|t| t.id == tenant.id)
            .ok_or_else(|| {
                PlatformError::new(ErrorCode::NotFound, "tenant not found".to_string())
            })?;
        if existing.revision != expected {
            return Err(PlatformError::new(
                ErrorCode::VersionMismatch,
                "revision conflict".to_string(),
            ));
        }
        *existing = tenant.clone();
        Ok(())
    }

    async fn delete(
        &self,
        _id: TenantId,
        _expected: Revision,
        _ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        Ok(())
    }

    async fn list(&self, _ctx: &RequestContext) -> Result<Page<Tenant>, PlatformError> {
        let tenants = self.tenants.lock().unwrap_or_else(|e| panic!("{e:?}"));
        Ok(Page {
            items: tenants.clone(),
            next_cursor: None,
        })
    }
}

#[derive(Debug, Clone)]
struct FakeUserRepo {
    users: Arc<Mutex<Vec<domain_identity::user::User>>>,
}

impl FakeUserRepo {
    fn new() -> Self {
        Self {
            users: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

#[async_trait]
impl UserRepository for FakeUserRepo {
    async fn by_id(
        &self,
        id: UserId,
        _ctx: &RequestContext,
    ) -> Result<domain_identity::user::User, PlatformError> {
        let users = self.users.lock().unwrap_or_else(|e| panic!("{e:?}"));
        users
            .iter()
            .find(|u| u.id == id)
            .cloned()
            .ok_or_else(|| PlatformError::new(ErrorCode::NotFound, "user not found".to_string()))
    }

    async fn by_username(
        &self,
        _username: &str,
        _ctx: &RequestContext,
    ) -> Result<domain_identity::user::User, PlatformError> {
        Err(PlatformError::new(
            ErrorCode::NotFound,
            "user not found".to_string(),
        ))
    }

    async fn create(
        &self,
        user: &domain_identity::user::User,
        _ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        let mut users = self.users.lock().unwrap_or_else(|e| panic!("{e:?}"));
        users.push(user.clone());
        Ok(())
    }

    async fn update(
        &self,
        user: &domain_identity::user::User,
        expected: Revision,
        _ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        let mut users = self.users.lock().unwrap_or_else(|e| panic!("{e:?}"));
        let existing = users
            .iter_mut()
            .find(|u| u.id == user.id)
            .ok_or_else(|| PlatformError::new(ErrorCode::NotFound, "user not found".to_string()))?;
        if existing.revision != expected {
            return Err(PlatformError::new(
                ErrorCode::VersionMismatch,
                "revision conflict".to_string(),
            ));
        }
        *existing = user.clone();
        Ok(())
    }

    async fn delete(
        &self,
        _id: UserId,
        _expected: Revision,
        _ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        Ok(())
    }

    async fn list(
        &self,
        _ctx: &RequestContext,
    ) -> Result<Page<domain_identity::user::User>, PlatformError> {
        let users = self.users.lock().unwrap_or_else(|e| panic!("{e:?}"));
        Ok(Page {
            items: users.clone(),
            next_cursor: None,
        })
    }
}

#[derive(Debug, Clone)]
struct FakeAuth {
    allow: bool,
}

#[async_trait]
impl AuthorizationPort for FakeAuth {
    async fn authorize(
        &self,
        _req: AuthorizationRequest,
        _ctx: &RequestContext,
    ) -> Result<AuthorizationResponse, PlatformError> {
        if self.allow {
            Ok(AuthorizationResponse {
                decision: Decision::Allow,
                binding_ids: vec![],
                reason: Reason::Allowed,
            })
        } else {
            Ok(AuthorizationResponse {
                decision: Decision::Deny,
                binding_ids: vec![],
                reason: Reason::NoBinding,
            })
        }
    }
}

#[derive(Debug, Clone)]
struct FakeAudit {
    records: Arc<Mutex<Vec<AuditRecord>>>,
    fail_action: Option<String>,
}

impl FakeAudit {
    fn new() -> Self {
        Self {
            records: Arc::new(Mutex::new(Vec::new())),
            fail_action: None,
        }
    }

    fn failing(action: impl Into<String>) -> Self {
        Self {
            records: Arc::new(Mutex::new(Vec::new())),
            fail_action: Some(action.into()),
        }
    }
}

#[async_trait]
impl AuditWriter for FakeAudit {
    async fn write(
        &self,
        record: &AuditRecord,
        _ctx: &RequestContext,
    ) -> Result<AuditRecordId, PlatformError> {
        if self.fail_action.as_ref() == Some(&record.action) {
            return Err(PlatformError::new(
                ErrorCode::Internal,
                "audit write failed".to_string(),
            ));
        }
        let mut records = self.records.lock().unwrap_or_else(|e| panic!("{e:?}"));
        let id = AuditRecordId(records.len() as i64 + 1);
        let mut record = record.clone();
        record = record.with_id(id);
        records.push(record);
        Ok(id)
    }
}

fn build_tenant_service(
    allow: bool,
) -> (
    TenantService<FakeTenantRepo, FakeAuth, FakeAudit, FakeClock, impl IdGenerator>,
    FakeClock,
    FakeTenantRepo,
    FakeAudit,
) {
    let clock = fake_clock();
    let repo = FakeTenantRepo::new();
    let auth = FakeAuth { allow };
    let audit = FakeAudit::new();
    let service = TenantService::new(repo.clone(), auth, audit.clone(), clock, id_gen(clock));
    (service, clock, repo, audit)
}

fn build_user_service(
    allow: bool,
) -> (
    UserService<FakeUserRepo, FakeAuth, FakeAudit, FakeClock, impl IdGenerator>,
    FakeClock,
    FakeUserRepo,
    FakeAudit,
) {
    let clock = fake_clock();
    let repo = FakeUserRepo::new();
    let auth = FakeAuth { allow };
    let audit = FakeAudit::new();
    let service = UserService::new(repo.clone(), auth, audit.clone(), clock, id_gen(clock));
    (service, clock, repo, audit)
}

#[tokio::test]
async fn tenant_create_succeeds_and_audit_is_written() {
    let (service, _clock, repo, audit) = build_tenant_service(true);
    let ctx = platform_ctx();
    let req = WriteRequest::for_create(CreateTenantRequest {
        code: "acme".to_string(),
        name: "Acme".to_string(),
        locale: None,
        timezone: None,
    });

    let resp = service
        .create(req, &ctx)
        .await
        .unwrap_or_else(|e| panic!("{e:?}"));
    assert_eq!(resp.data.code, "acme");
    assert_eq!(resp.revision.value(), 1);

    let tenants = repo.tenants.lock().unwrap_or_else(|e| panic!("{e:?}"));
    assert_eq!(tenants.len(), 1);

    let records = audit.records.lock().unwrap_or_else(|e| panic!("{e:?}"));
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].action, "tenant.create");
}

#[tokio::test]
async fn tenant_create_is_denied_without_authorization() {
    let (service, _clock, _repo, _audit) = build_tenant_service(false);
    let ctx = platform_ctx();
    let req = WriteRequest::for_create(CreateTenantRequest {
        code: "acme".to_string(),
        name: "Acme".to_string(),
        locale: None,
        timezone: None,
    });

    let err = match service.create(req, &ctx).await {
        Err(e) => e,
        Ok(_) => panic!("expected denied error"),
    };
    assert_eq!(err.code(), ErrorCode::Denied);
}

#[tokio::test]
async fn tenant_update_fails_on_revision_conflict() {
    let (service, clock, repo, _audit) = build_tenant_service(true);
    let ctx = platform_ctx();
    let create_req = WriteRequest::for_create(CreateTenantRequest {
        code: "acme".to_string(),
        name: "Acme".to_string(),
        locale: None,
        timezone: None,
    });
    let resp = service
        .create(create_req, &ctx)
        .await
        .unwrap_or_else(|e| panic!("{e:?}"));

    let update_req = WriteRequest {
        expected_revision: Some(Revision::new(999)),
        idempotency: None,
        payload: application::tenant::UpdateTenantRequest {
            id: resp.data.id,
            name: "Updated".to_string(),
            locale: None,
            timezone: None,
        },
    };
    let err = match service.update(update_req, &ctx).await {
        Err(e) => e,
        Ok(_) => panic!("expected version mismatch error"),
    };
    assert_eq!(err.code(), ErrorCode::VersionMismatch);

    let tenants = repo.tenants.lock().unwrap_or_else(|e| panic!("{e:?}"));
    assert_eq!(tenants[0].name, "Acme");
    assert!(clock.now().timestamp_millis() > 0);
}

#[tokio::test]
async fn tenant_create_is_cancelled_when_deadline_expired() {
    let clock = fake_clock();
    let repo = FakeTenantRepo::new();
    let auth = FakeAuth { allow: true };
    let audit = FakeAudit::new();
    let service = TenantService::new(repo, auth, audit, clock, id_gen(clock));

    let mut ctx = platform_ctx();
    ctx.deadline = Some(Deadline::new(
        UtcTimestamp::parse_rfc3339("2000-01-01T00:00:00Z").unwrap_or_else(|e| panic!("{e:?}")),
    ));

    let req = WriteRequest::for_create(CreateTenantRequest {
        code: "acme".to_string(),
        name: "Acme".to_string(),
        locale: None,
        timezone: None,
    });

    let err = match service.create(req, &ctx).await {
        Err(e) => e,
        Ok(_) => panic!("expected cancelled error"),
    };
    assert_eq!(err.code(), ErrorCode::Cancelled);
}

#[tokio::test]
async fn tenant_create_fails_when_audit_write_fails() {
    let clock = fake_clock();
    let repo = FakeTenantRepo::new();
    let auth = FakeAuth { allow: true };
    let audit = FakeAudit::failing("tenant.create");
    let service = TenantService::new(repo, auth, audit, clock, id_gen(clock));

    let ctx = platform_ctx();
    let req = WriteRequest::for_create(CreateTenantRequest {
        code: "acme".to_string(),
        name: "Acme".to_string(),
        locale: None,
        timezone: None,
    });

    let err = match service.create(req, &ctx).await {
        Err(e) => e,
        Ok(_) => panic!("expected internal error"),
    };
    assert_eq!(err.code(), ErrorCode::Internal);
}

#[tokio::test]
async fn user_create_succeeds_in_tenant_context() {
    let tenant_id = TenantId::parse_str("018e1234-5678-7abc-8def-0123456789ab")
        .unwrap_or_else(|e| panic!("{e:?}"));
    let (service, _clock, repo, audit) = build_user_service(true);
    let ctx = tenant_ctx(tenant_id);
    let req = WriteRequest::for_create(CreateUserRequest {
        username: "alice".to_string(),
        display_name: "Alice".to_string(),
    });

    let resp = service
        .create(req, &ctx)
        .await
        .unwrap_or_else(|e| panic!("{e:?}"));
    assert_eq!(resp.data.username, "alice");
    assert_eq!(resp.data.tenant_id, tenant_id);

    let users = repo.users.lock().unwrap_or_else(|e| panic!("{e:?}"));
    assert_eq!(users.len(), 1);

    let records = audit.records.lock().unwrap_or_else(|e| panic!("{e:?}"));
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].action, "user.create");
}

#[tokio::test]
async fn write_request_idempotency_context_can_be_attached() {
    let req = WriteRequest::for_create(CreateTenantRequest {
        code: "acme".to_string(),
        name: "Acme".to_string(),
        locale: None,
        timezone: None,
    })
    .with_idempotency(IdempotencyContext {
        key: "key-1".to_string(),
        endpoint: "tenant.create".to_string(),
        digest: "digest-1".to_string(),
    });
    if let Some(idempotency) = &req.idempotency {
        assert_eq!(idempotency.key, "key-1");
    } else {
        panic!("idempotency context missing");
    }
}
