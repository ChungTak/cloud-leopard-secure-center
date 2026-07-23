# 21. 安全、可观测与运维

### SEC-001：威胁模型和安全回归
**前置：** VID-003、PLG-003、ALM-002。
- [x] `observability/src/security.rs` 定义 `ThreatCategory`（tenant 越权/ID 混淆/token replay/旧 epoch/插件越权/URL 泄漏/SSRF/审计篡改）、`SecurityControl`、`RiskLevel` 与 `ThreatControlMatrix`。
- [x] `SecurityAssessor` port、`UnsupportedSecurityAssessor` stub，以及 `mtls_identity_matches` 占位；无配置返回 `Unavailable`，有配置返回 `Unsupported`。
- [x] 控制项包含 `owner`、`test_ref`、`residual_risk`；mTLS/证书轮换与出站/解析器/权限控制在真实实现后补全矩阵。

### OBS-001：日志、指标和追踪
**前置：** FND-002、MSG-004。
- [x] `observability/src/telemetry.rs` 定义 `TraceContext`（traceparent 解析）、`TelemetryConfig`、`TelemetryInitializer` port、`MetricRegistry`（安全 label 白名单）与 `UnsupportedTelemetryInitializer` stub。
- [x] `MetricRegistry` 拒绝 `user_id` 等高基数 label；`redact` 统一脱敏为 `[REDACTED]`。
- [x] 未配置 exporter 返回 `Unavailable`；有配置返回 `Unsupported`。真实 OpenTelemetry/tracing、SLO dashboard 与跨 HTTP/UoW/Outbox/NATS/signaling/plugin 上下文传播在后续实现。

### OBS-002：健康、告警与 runbook
**前置：** OBS-001。
- [x] `observability/src/health.rs` 定义 `HealthState`、`RoleReadiness`、`AlertRule`、`Runbook`（含 diagnosis/mitigation/recovery/rollback、`avoid_deletion_first`）、`HealthMonitor` port 与 `UnsupportedHealthMonitor` stub。
- [x] readiness 按角色依赖占位；DB/NATS/signaling/projection/disk/cert 告警规则在真实 monitor 实现后填充。
- [x] runbook 数据结构显式设置 `avoid_deletion_first=true`；节点 drain/证书过期/积压/磁盘满演练场景后续实现。

