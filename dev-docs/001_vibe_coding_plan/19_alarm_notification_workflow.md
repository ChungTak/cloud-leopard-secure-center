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
- [x] `domain-alarm/src/linkage.rs` 定义 `LinkageCondition`、`LinkageAction`、`AlarmLinkageRule`（含 cooldown、max_depth、exclusions）、`LinkageOutcome`（Success/UnknownOutcome）与 `LinkageWorkflow` port。
- [x] `UnsupportedLinkageWorkflow` stub：未配置返回 `Unavailable`，已配置返回 `Unsupported`。高风险动作二次授权、workflow attempt 审计写入和可重放执行在后续实现。
- [x] 联动 workflow 与告警、通知模块解耦，不修改 `Alarm` 权威状态。

