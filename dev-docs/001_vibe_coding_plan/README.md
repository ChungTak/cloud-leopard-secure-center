# Cloud Leopard Secure Center 编程执行计划

## 1. 定位

本目录把 [`docs/system-design.md`](../../docs/system-design.md) 转换为可由编程智能体直接执行的原子任务。正式方案定义“为什么”和系统边界，本计划定义“按什么顺序、修改什么、如何证明完成”。

执行体不得依赖未写入本文档集的隐含决定。设计与计划冲突时，以正式方案为准并停止受影响任务；先修订设计或新增 ADR，再继续实现。

## 2. 全局执行纪律

1. 开始任务前阅读本 README、[执行协议](01_execution_contract_and_baseline.md)、任务所在专题及全部前置任务报告。
2. 每个 `[ ]` 任务原则上对应一个可独立评审提交。不得顺手实现后续任务。
3. 先写契约和失败测试，再写领域实现、adapter 和集成测试。
4. 禁止生产路径出现 `todo!()`、`unimplemented!()`、空 provider、固定成功返回、吞错或无界资源。
5. 未实现能力返回稳定 `UNSUPPORTED`；不允许伪造可用状态。
6. 所有外部调用同时定义 deadline、取消、错误映射、幂等和遥测。
7. 所有集合、队列、缓存、分页、重试、批次和并发都有配置上限及过载测试。
8. 完成任务后新增 `reports/<task-id-lowercase>.md`，记录提交、变更、命令和结果，再把 `[ ]` 改为 `[x]`。
9. 不修改无关用户文件，不直接修改 cheetah 上游仓库，不绕过测试或提交钩子。

## 3. 固定技术基线

| 项目 | 基线 |
| --- | --- |
| Rust | 1.96.1、Edition 2024、resolver 3 |
| Tokio/Axum/Tower | 1.52.3 / 0.8.9 / 0.5.3 |
| SQLx | 0.8.6，PostgreSQL only |
| Tonic/Prost | 0.14 / 0.14 |
| async-nats | 0.49.1 |
| Node/pnpm | 22.22.2 / 11.12.0 |
| React | 19.2.8 |
| TypeScript/Vite | 7.0.2 / 8.1.4 |
| Semi Design | 2.101.1 |
| Vitest/Playwright | 4.1.10 / 1.61.1 |
| 上游 signaling | commit `cfe35952c33279fd3f31b605ac053ff5c725814c` |
| 上游 media-engine | commit `49531f6f863840e7c4211bd66917c9711abf3305` |

依赖由 lockfile 固定。变更基线必须单独提交，附兼容性、许可证和回归报告。

## 4. 阶段索引

| Phase | 文档 | 交付 |
| --- | --- | --- |
| 0 | [01](01_execution_contract_and_baseline.md)–[05](05_postgres_migrations_rls.md) | 执行协议、workspace、架构、基础类型、PostgreSQL |
| 1 | [06](06_identity_and_sessions.md)–[15](15_phase1_acceptance_packaging.md) | 管理核心闭环和首期发布 |
| 2 | [16](16_signaling_contract_and_projection.md)–[17](17_video_entitlement_and_player.md) | 信令资源投影、实时/回放和 Web 播放器 |
| 3 | [18](18_nats_roles_cluster_runtime.md)–[20](20_plugin_sdk_wit_grpc.md) | NATS、告警、插件 |
| 4 | [21](21_security_observability_operations.md)–[23](23_release_upgrade_disaster_recovery.md) | 生产加固、规模验证、发布灾备 |
| Upstream | [90](90_cheetah_signaling_upstream_requirements.md)、[91](91_cheetah_media_engine_upstream_requirements.md) | 可移交上游契约需求 |

专题清单：

- [02 Workspace、工具链与 CI](02_workspace_toolchain_and_ci.md)
- [03 架构与 crate 图](03_architecture_crate_graph.md)
- [04 基础类型、错误与配置](04_foundation_types_errors_config.md)
- [05 PostgreSQL、迁移与 RLS](05_postgres_migrations_rls.md)
- [06 身份与会话](06_identity_and_sessions.md)
- [07 组织与空间树](07_organization_and_spatial_tree.md)
- [08 RBAC 与资源范围](08_authorization_rbac_scope.md)
- [09 资源目录、绑定与投影](09_resource_catalog_bindings_projection.md)
- [10 审计、配置与密钥](10_audit_configuration_secret.md)
- [11 应用服务、UoW、Outbox 与任务](11_application_uow_outbox_jobs.md)
- [12 HTTP、OpenAPI 与认证](12_http_openapi_authentication.md)
- [13 前端基础与 Design System](13_frontend_foundation_and_design_system.md)
- [14 管理功能页面](14_frontend_management_features.md)
- [15 首期验收与打包](15_phase1_acceptance_packaging.md)
- [16 Signaling 契约与投影](16_signaling_contract_and_projection.md)
- [17 视频授权与播放器](17_video_entitlement_and_player.md)
- [18 NATS、角色与集群运行时](18_nats_roles_cluster_runtime.md)
- [19 告警、通知与联动](19_alarm_notification_workflow.md)
- [20 插件 SDK、WIT 与 gRPC](20_plugin_sdk_wit_grpc.md)
- [21 安全、可观测与运维](21_security_observability_operations.md)
- [22 测试、性能与混沌](22_testing_performance_chaos.md)
- [23 发布、升级与灾备](23_release_upgrade_disaster_recovery.md)

## 5. 关键依赖路径

```text
BAS -> ARC -> FND -> DB
                  ├-> IAM -> API -> WEB
                  ├-> ORG -> AUTH -> RES
                  └-> APP -> AUD

Phase 1 accepted
  -> SIG -> VID
  -> MSG -> ALM
  -> MSG -> PLG
  -> SEC/OBS -> TST -> PKG
```

同一层任务只有在专题明确写出可并行时才能并行。数据库 schema、公共错误、公开 DTO、Proto 和 OpenAPI 由单一任务负责，其他任务不得抢先修改。

## 6. 全局完成定义

- [ ] `BAS` 至 `PKG` 所有必选任务完成，无未登记 TODO。
- [ ] 正式方案每条要求均映射到本目录任务。
- [ ] Cargo、OpenAPI、Proto、SQL migration 和前端生成物可重复生成。
- [ ] PostgreSQL RLS、revision、幂等、Outbox/Inbox contract 全通过。
- [ ] Local/NATS 和 fake/real signaling adapter 通过同一 contract suite。
- [ ] 管理核心、视频、告警和插件各有端到端闭环。
- [ ] 单机、角色化集群、升级、回滚和灾备演练有可复现报告。
- [ ] 所有上游缺口已关闭或以稳定 `UNSUPPORTED` 明确排除。
- [ ] `cargo fmt/clippy/nextest/deny`、前端 typecheck/test/build、OpenAPI/Buf breaking 和安全扫描通过。

## 7. 设计覆盖矩阵

| 正式方案主题 | 执行文档 |
| --- | --- |
| 分层、crate 图和角色 | 02–04、18 |
| Tenant/Organization/Identity/Authz | 05–08、12–14 |
| Resource Catalog 与数据权威 | 09、16 |
| Audit/Configuration/Secret | 10、21 |
| PostgreSQL/RLS/migration | 05、11、23 |
| REST/OpenAPI/错误/幂等 | 04、11–12 |
| Outbox/Inbox/NATS/KV | 11、18 |
| cheetah-signaling | 16、18、90 |
| 视频与 media-engine | 17、91 |
| 告警与插件 | 19–20 |
| 安全、可观测、测试 | 21–22 |
| 打包、升级、灾备 | 15、23 |

## 8. 报告与状态

报告规范见 [`reports/README.md`](reports/README.md)。任务状态只由专题 checkbox 和对应报告共同构成；只有报告没有 checkbox，或只有 checkbox 没有报告，都视为未完成。
