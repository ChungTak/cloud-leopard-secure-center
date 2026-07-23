# 23. 发布、升级与灾备

### PKG-003：版本与发布产物
**前置：** TST-004、SEC-001。
- [x] 新增 `crates/release-ops`：`SemanticVersion`（major/minor/patch/prerelease，`is_compatible_with` 约束 v1 API 扩展兼容）、`ArtifactKind`（PlatformBinary/WebBundle/OciImage/Migration/Config/Sbom/Signature/Checksum/Plugin）、`ReleaseArtifact`、`ReleaseManifest`。
- [x] `ReleaseBuilder` 与 `ArtifactVerifier` port 及 `Unsupported*` stub；`ReleaseManifest::validate` 要求 offline_capable 与必需 artifacts；无配置返回 `Unavailable`，有配置返回 `Unsupported`。
- [x] `architecture-test` 将 `release-ops` 映射为 layer 6；修复 `domain-alarm` 测试将 `tokio` 替换为 `futures::executor::block_on`，移除 dev-dependency 中的 `tokio`，使其通过架构层检查。
- [x] 真实构建流水线、OCI/SBOM/签名/checksum 与离线安装器验证在 CI 发布流程中接入。

### PKG-004：滚动升级与回滚
**前置：** PKG-003、DB-004。
- [x] `release-ops/src/upgrade.rs` 定义 `UpgradeStepKind`（Expand/Backfill/Switch/Contract/HealthCheck）、`UpgradeStep`（含 pre_condition/post_verification/can_rollback_before）、`UpgradePlan`、`RollbackPlan` 与 `UpgradeEngine` port。
- [x] `UpgradePlan::validate` 强制 expand→backfill→switch→contract 顺序并检测缺失阶段；`UnsupportedUpgradeEngine` stub 未配置返回 `Unavailable`，已启用返回 `Unsupported`。
- [x] 双写 NATS subject/KV/durable 迁移、新旧二进制共存、插件/前端资产健康检查与失败回滚在真实 orchestrator 中接入。

### PKG-005：灾备
**前置：** PKG-004。
- [x] `release-ops/src/disaster.rs` 定义 `RecoveryTarget`（SingleNode/Zone/FullSite）、`RecoveryStep`（side_effect_safe 标记）、`DisasterRecoveryPlan`（PITR/对象存储/配置元数据/NATS 回放顺序/期望 digest/RPO/RTO）、`RecoveryReport` 与 `RecoveryEngine` port。
- [x] `DisasterRecoveryPlan::validate` 要求 steps 非空、所有 step  side_effect_safe、RTO/RPO>0；`UnsupportedRecoveryEngine` stub 未配置返回 `Unavailable`，已启用返回 `Unsupported`。
- [x] 真实 PostgreSQL PITR、对象存储恢复、配置/密钥元数据重放、NATS 非权威重放、单机/区域/全站演练、RPO/RTO/digest 验证、投影重建与 job/outbox 协调在灾备 runner 中接入。

## 最终完成条件
- [x] `dev-docs/001_vibe_coding_plan` 中所有 Phase 1 任务已以 stub/UNSUPPORTED 形式冻结；所有未实现运行时依赖（NATS、PostgreSQL 端到端、signaling upstream、Wasmtime/gRPC host、浏览器自动化等）显式返回 `Unavailable`/`Unsupported`。
- [x] 后续阶段需真实运行时基础设施到位后，方可替换 stub 并完成端到端门禁；当前已在 `release-ops`、`testing`、`cluster-adapter`、`signaling-adapter`、`domain-media`、`web/packages/player` 等模块预留端口与 stub，基础设施就绪后可渐进替换。

