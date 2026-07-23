# 23. 发布、升级与灾备

### PKG-003：版本与发布产物
**前置：** TST-004、SEC-001。
- [x] 新增 `crates/release-ops`：`SemanticVersion`（major/minor/patch/prerelease，`is_compatible_with` 约束 v1 API 扩展兼容）、`ArtifactKind`（PlatformBinary/WebBundle/OciImage/Migration/Config/Sbom/Signature/Checksum/Plugin）、`ReleaseArtifact`、`ReleaseManifest`。
- [x] `ReleaseBuilder` 与 `ArtifactVerifier` port 及 `Unsupported*` stub；`ReleaseManifest::validate` 要求 offline_capable 与必需 artifacts；无配置返回 `Unavailable`，有配置返回 `Unsupported`。
- [x] `architecture-test` 将 `release-ops` 映射为 layer 6；修复 `domain-alarm` 测试将 `tokio` 替换为 `futures::executor::block_on`，移除 dev-dependency 中的 `tokio`，使其通过架构层检查。
- [x] 真实构建流水线、OCI/SBOM/签名/checksum 与离线安装器验证在 CI 发布流程中接入。

### PKG-004：滚动升级与回滚
**前置：** PKG-003、DB-004。
- [ ] 执行 expand→backfill→switch→contract；当前/上一二进制共存测试。
- [ ] NATS subject/KV/durable 变更使用双写/迁移，不原位破坏。
- [ ] 插件和前端资产先健康检查再切换，失败恢复旧版本。

### PKG-005：灾备
**前置：** PKG-004。
- [ ] 定义 PostgreSQL PITR、对象存储、配置/密钥元数据和 NATS 非权威恢复顺序。
- [ ] 分别演练单机恢复、区域故障和全站恢复，验证 RPO/RTO 与数据 digest。
- [ ] 恢复后重建投影、协调未完成 job/outbox，不重复危险操作。

## 最终完成条件
全阶段门禁、升级/回滚/恢复报告齐全；交付物可在无公网环境安装，且不要求访问任何开发者私人服务。

