# 11. 应用服务、UoW、Outbox/Inbox 与任务

### APP-001：应用用例模板
**前置：** IAM-001、AUTH-003、DB-003。
- [x] `crates/application/src/usecase.rs` 固定顺序：`check_deadline` → `require_actor` → `authorize_or_fail` → load aggregate → domain method → `repo.create/update` → `audit_write` → `WriteResponse`。
- [x] `TenantService`/`UserService`/`OrganizationService`/`RoleService`/`DeviceService`/`ConfigService` 在 `crates/application/src/` 实现对应 ports，不直接操作 SQL。
- [x] `WriteRequest<T>` 包含 `expected_revision` 与 `idempotency`；所有 mutating use case 均接受该结构。
**测试：** 集成测试覆盖成功、鉴权拒绝、乐观锁冲突、deadline 取消、审计失败。

### APP-002：Idempotency 与 Outbox
**前置：** APP-001。
- [x] `IdempotencyRecord` 唯一键为 `(tenant_id, principal_id, endpoint_scope, idempotency_key)`；保存 `request_digest` 与首次响应（`response_status`、`response_body`）。
- [x] `IdempotencyRepository` 新增 `save_or_conflict`：通过 `SELECT ... FOR UPDATE` 原子检查；相同 key 不同 digest 返回 `ErrorCode::Conflict`。
- [x] `UnitOfWork` port（`storage-api`）与 `PostgresUnitOfWork`（`storage-postgres`）支持聚合、审计、outbox 原子提交；`OutboxRepository::claim`/`mark_published` 提供有界 batch publisher 语义。
**测试：** `storage-postgres` 集成测试覆盖 100 次重复、commit/rollback 原子性、双 publisher 仅一次成功。

### APP-003：Inbox 与后台任务
**前置：** APP-002。
- [x] `InboxRepository::receive` 使用 `ON CONFLICT DO NOTHING` 按 `(tenant_id, consumer_id, message_id)` 去重；非首次返回已保存记录（含 `result_digest`）；`expires_at` 覆盖最大重放窗口。
- [x] `Job` 与 `JobRepository` 定义 `status`/`revision`/`lease_owner`/`lease_until`/`attempts`/`next_run`/`max_attempts`；`claim` 要求 `attempts < max_attempts` 且 `lease_until <= now`；`complete`/`fail` 校验 `lease_owner` 与 `revision`，过期 lease 被拒绝。
- [x] `foundation::retry::RetryPolicy` 提供指数退避 + 有界 jitter + `max_attempts` + 总 deadline；仅对 transient error 计算 `next_retry`。
**测试：** `storage-postgres` 集成测试覆盖 lease 竞争、重试次数上限、任务取消、过期租约回收。

