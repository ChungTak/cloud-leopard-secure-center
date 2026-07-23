# 19. 告警、通知与联动

### ALM-001：告警聚合与存储
**前置：** MSG-002、AUTH-003。
- [x] 新增 `crates/domain-alarm`：定义 `AlarmState`（NEW→ACKNOWLEDGED→PROCESSING→RESOLVED→CLOSED，外加 SUPPRESSED/MERGED/REOPENED）、`Severity`、`AlarmAction`、`EvidenceRef`、`DedupKey`、`AlarmEvent`、`Alarm` aggregate。
- [x] `Alarm` 支持新建、ack/assign/resolve/close/reopen/suppress/merge 状态机，以及 `UnsupportedAlarmRepository` port stub。
- [x] `foundation` 新增 `AlarmId`。持久化/租户上限/并发越权/证据损坏在后续 `domain-alarm` Postgres 实现中继续。
**测试：** alarm 起始状态、ack 状态迁移、非法 close、`UnsupportedAlarmRepository` 返回 unsupported。

### ALM-002：通知
**前置：** ALM-001、APP-003。
- [x] `domain-alarm/src/notification.rs` 定义 `NotificationChannel`（InApp/SSE/Webhook）、`Notification`（含 deadline、模板变量白名单校验）、`NotificationPort` 与 `UnsupportedNotificationPort` stub。
- [x] 模板变量通过 `validate_vars` 白名单过滤；无配置返回 `Unavailable`，有配置返回 `Unsupported`。幂等、退避、熔断、DLQ 与 SSRF/DNS rebinding 防护留到真实 delivery 实现。
- [x] 通知 port 与告警 aggregate 解耦，通知失败不会回滚 `Alarm` 权威状态。

### ALM-003：联动工作流
**前置：** ALM-002、PLG-002。
- [ ] 条件、动作、冷却、最大派生深度和 loop detection 显式配置。
- [ ] 高风险设备动作再次授权；无法确认结果为 UNKNOWN_OUTCOME。
- [ ] 每步写 workflow attempt 和审计，可重放但不重复副作用。

