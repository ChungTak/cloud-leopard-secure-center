# 03. 架构、crate 图与角色装配

## 1. 冻结依赖方向

```text
apps -> transport adapters -> application -> domain -> foundation
```

domain 禁止依赖 Tokio、Axum、Tonic、SQLx、NATS、HTTP/Proto DTO。application 只依赖 port trait；adapter 实现 port；apps 只装配。

### ARC-001：建立 crate 图

**前置：** BAS-003。

- [x] 创建 foundation、六个首期 domain、application、storage-api/postgres、message-api/local、http-api、observability。
- [x] signaling、message-nats、plugin crates 到对应阶段再加入 members，避免空 skeleton 冒充能力。
- [x] 所有依赖通过构造器注入；禁止 service locator、全局 mutable singleton。
- [x] 公共类型只从其权威 crate 导出，禁止循环 re-export。

**验收：** `cargo metadata` 生成图与本文件一致；domain feature 组合不能引入 adapter。

### ARC-002：架构自动检查

**前置：** ARC-001。

- [x] 建立 architecture-test，读取 metadata 并验证层级、禁用依赖和 crate 前缀。
- [x] 检查 app 源码不出现 SQL query、业务 aggregate 或协议解析器。
- [x] 检查 domain 不出现 framework crate 和生成 DTO。
- [x] 用故意违规 fixture 证明检查有效。

### ARC-003：角色化装配

**前置：** ARC-001、FND-003。

- [x] `cluster-adapter/src/lib.rs` 定义 `Role = Api | Workflow | Projection | Scheduler | PluginHost | All`，并实现 `Role::expand`：`All` 展开为 `[Api, Workflow, Projection, Scheduler, PluginHost]`。
- [x] `cluster-adapter/src/assembly.rs` 定义 `LifecyclePhase` 与 `Lifecycle`；`All` 角色在单进程中由 `ClusterAssembler` 统一处理；未配置 NATS 时 `run`/`ready`/`shutdown` 返回 `Unavailable`，已配置时返回 `Unsupported`，不假 ready。
- [x] `Lifecycle::startup()` 固定顺序 `config/secret → schema check → bus → repositories → workers → listeners → ready`。
- [x] `Lifecycle::shutdown()` 固定反向顺序并在末尾执行 `Drain`；`validate_startup`/`validate_shutdown` 检测缺失/重复阶段与 drain 收尾。

**测试：** 每个角色缺少必需依赖时启动失败；取消后无残留 task/listener。

## 2. 完成条件

- crate README 与实际依赖一致；
- 架构测试进入 PR 门禁；
- app binary 无领域业务；
- 无为后续阶段创建的固定成功 provider。

