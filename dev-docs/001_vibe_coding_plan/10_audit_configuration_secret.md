# 10. 审计、配置与密钥

### AUD-001：追加写审计
**前置：** DB-004、FND-002。
- [x] 建立月分区 audit.records 和专用 writer；业务账号无 UPDATE/DELETE。
- [x] 记录 actor、tenant、action、target、result、request/trace、IP、前后 digest；details 有 schema/大小限制。
- [x] 成功与拒绝的高风险操作均审计；审计失败按 action 风险选择拒绝或告警，不静默丢失。
**测试：** 修改/删除被 DB 拒绝、分区路由、脱敏、审计 writer 故障策略。

### AUD-002：配置定义和值
**前置：** FND-003、DB-003。
- [x] Definition 固定类型/schema/default/sensitive/dynamic；Value 按 platform/tenant/module scope 唯一。
- [x] 解析优先级明确；非法新值不替换旧快照。
- [x] sensitive definition 只允许 secret_ref，API 永不返回 resolved value。
**测试：** schema、scope、revision、动态 reload、secret redaction。

### AUD-003：保留与清理
**前置：** AUD-001。
- [x] `domain-audit/src/retention.rs` 为 `AuditRecords`/`AuditEvents`/`LoginAttempts`/`Outbox`/`Inbox` 定义默认保留（365/90/30/7/7 天）与 `TenantRetentionOverride`；`storage-postgres/src/retention_repository.rs` 实现 `get_effective_days`（tenant override 优先）。
- [x] `RetentionRepository` 提供 `acquire_lease`/`release_lease`、`cleanup_batch`（ honoring legal holds via `audit.cleanup_batch` SQL 函数）与 `list_partitions_to_clean`；legal hold 资源通过 `add_legal_hold`/`remove_legal_hold` 管理，清理函数自动跳过。
- [x] `RetentionRepository::drop_partition` 在 `backup_confirmed=false` 时返回 `Invalid`，在已确认时写入 `partition_drop` 审计记录并返回 `Unsupported`；真实分区 detach 与备份编排留到运维环境。
**测试：** 中断恢复、双 worker、hold、磁盘接近上限。

