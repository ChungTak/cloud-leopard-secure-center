# 01. 编程智能体执行协议

## 1. 目标

把任务执行方式固定下来，使新的执行体无需猜测工作边界、完成标准或证据格式。

## 2. 每次任务的固定流程

1. 读取 `README.md`、本专题、所有前置任务报告和适用 `AGENTS.md`。
2. 运行 `git status --short`，识别并保留用户已有改动。
3. 在报告中记录任务 ID、基线 commit、前置任务状态和预计修改路径。
4. 先提交失败测试或契约 snapshot，再实现最小闭环。
5. 对每个外部边界补齐 timeout、cancel、limit、error、metrics 和 secret redaction。
6. 运行任务要求的精确命令及受影响的上游测试。
7. 检查没有新增 placeholder、无界资源、敏感数据或架构违规。
8. 创建任务报告，最后更新 checkbox；失败时保持 `[ ]`。

## 3. 变更边界

- 一个任务只修改其“允许路径”；必须跨边界时先拆出前置任务。
- 公共接口变化必须同时更新契约、兼容测试、文档和调用方。
- 已发布 migration 只能追加，禁止修改历史文件。
- 生成文件只能通过固定脚本生成，禁止手工修补。
- 不把 HTTP/SQL/Proto DTO 作为 domain type。
- 不通过 feature、re-export 或 type alias 隐藏违规依赖。
- 不提交真实密码、token、私钥、个人信息或未脱敏报文。

## 4. 失败与阻塞

遇到以下情况立即停止受影响任务并写报告：

- 正式设计与执行计划冲突；
- 前置任务、公开契约或上游产物缺失；
- 用户改动与任务目标重叠且无法安全合并；
- 测试揭示需要改变数据所有权、REST、Proto、NATS ABI 或安全模型；
- 只能通过假实现、跳过测试或扩大权限继续。

报告必须说明已验证事实、阻塞点、最小解除条件和未执行内容，不得自行选择“最省事”的语义。

## 5. 任务条目模板

每个任务包含：

```text
ID / 目标 / 前置 / 允许路径 / 禁止范围
公开契约或数据变更
实施步骤（有序）
测试矩阵（成功、失败、边界、并发、恢复）
验证命令
完成条件
```

## 6. 提交规则

- commit message：`<task-id>: <imperative summary>`；
- 不使用 `--no-verify`；
- 不把格式化或无关重构混入功能提交；
- 每个提交可在干净 checkout 上独立构建和测试；
- checkbox 后追加：`完成：<commit>；报告：reports/<task-id-lowercase>.md`。

## 7. 基线任务

### BAS-001：记录环境与设计基线

**前置：** 无。  
**允许路径：** 工具链文件、`docs/`、本计划 reports。  
**禁止：** 创建业务 skeleton。

- [ ] 记录 OS、架构、Rust/Node/容器工具版本和缺失工具。
- [ ] 记录正式方案 hash、两个 cheetah commit、许可证和契约产物可用性。
- [ ] 建立“设计决策不可覆盖清单”和需求追踪表。
- [ ] 验证外部依赖可获取；离线环境则记录镜像/包缓存要求。

**验收：** 报告包含可复制命令；另一执行体能得到同一基线结论。

### BAS-002：建立任务状态检查器

**前置：** BAS-001。  
**允许路径：** `tools/plan-check/`、CI 配置、本计划。

- [ ] 检查任务 ID 唯一、前置 ID 存在、依赖无环。
- [ ] 检查 `[x]` 任务存在对应报告和 commit 字段。
- [ ] 检查相对链接、代码围栏和检查器内置的占位表达式列表。
- [ ] 为成功和故意失败 fixture 编写测试。

**验收命令：** `cargo run -p plan-check -- dev-docs/001_vibe_coding_plan` 或等价锁定脚本。

## 8. 全局验证最低集

任务完成前至少运行受影响集合：

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
cargo deny check
pnpm typecheck
pnpm test
pnpm build
```

未建立对应 workspace 前，在报告中标为“由前置任务提供”，不能伪造通过。
