# 15. 第一阶段验收与打包

### PKG-001：单机 Compose 与安装检查
**前置：** API-003、WEB-007、APP-003。
- [x] 构建平台镜像和静态 Web 资源；非 root、只读 rootfs、healthcheck。
- [x] Compose 包含 platform/PostgreSQL/OTel，可使用外部数据库；secret 不写默认文件。
- [x] 安装前检查端口、磁盘、时钟、数据库版本、备份目录和证书。
**测试：** 全新离线安装、重启、错误配置、数据库暂不可用、升级前备份。

### TST-001：Phase 1 端到端验收
**前置：** PKG-001。
- [x] 在 `apps/security-platform/tests/phase1_acceptance.rs` 中自动化 Phase 1 契约验收：验证 closure、scope、revision、幂等、RLS、审计和 tenant cache 相关 stub 与 typed ID 行为。
- [x] 测试不依赖真实 PostgreSQL 与浏览器；无上游 signaling/media 时所有未实现能力显式返回 `Unsupported`/`Unavailable`。
- [x] 测试报告由 `cargo nextest` 输出，记录 commit 与本地运行环境。

### PKG-002：首期发布门禁
**前置：** TST-001。
- [x] 新增 `scripts/release-gate.sh`，运行 fmt/clippy/nextest/deny/aarch64、Web 检查、OpenAPI snapshot 差异、migration info、容器构建、SBOM（syft，未安装时标记 `UNSUPPORTED`）、许可证扫描（cargo deny / trivy，未安装时标记 `UNSUPPORTED`）。
- [x] 新增 `RELEASE.md` 说明 release 流程、产物、校验和、升级/回滚步骤，并列出 0.1.0 中显式 `UNSUPPORTED` 的能力。
- [x] Workspace 版本已设置为 `0.1.0`；release gate 生成 `dist/release-0.1.0.md` 作为版本产物入口。

## 完成条件
单个业务二进制与 PostgreSQL 完成管理闭环；signaling/media/plugin 未实现能力明确返回 `UNSUPPORTED`，无占位成功。
