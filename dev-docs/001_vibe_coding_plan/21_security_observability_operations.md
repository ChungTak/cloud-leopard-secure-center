# 21. 安全、可观测与运维

### SEC-001：威胁模型和安全回归
**前置：** VID-003、PLG-003、ALM-002。
- [x] `observability/src/security.rs` 定义 `ThreatCategory`（tenant 越权/ID 混淆/token replay/旧 epoch/插件越权/URL 泄漏/SSRF/审计篡改）、`SecurityControl`、`RiskLevel` 与 `ThreatControlMatrix`。
- [x] `SecurityAssessor` port、`UnsupportedSecurityAssessor` stub，以及 `mtls_identity_matches` 占位；无配置返回 `Unavailable`，有配置返回 `Unsupported`。
- [x] 控制项包含 `owner`、`test_ref`、`residual_risk`；mTLS/证书轮换与出站/解析器/权限控制在真实实现后补全矩阵。

### OBS-001：日志、指标和追踪
**前置：** FND-002、MSG-004。
- [ ] tracing 上下文贯穿 HTTP/UoW/Outbox/NATS/signaling/plugin；使用 W3C trace context。
- [ ] 高基数 ID 不作 Prometheus label；遥测失败不阻塞业务。
- [ ] 实现正式方案指标和 SLO dashboard；所有 secret/url/header 统一脱敏。

### OBS-002：健康、告警与 runbook
**前置：** OBS-001。
- [ ] live/ready 按角色依赖；定义 DB/NATS/signaling/projection/disk/cert 告警。
- [ ] runbook 包含判断、止损、恢复、验证和升级路径，不建议删除数据作为首选动作。
- [ ] 演练节点 drain、证书过期、积压和磁盘满。

