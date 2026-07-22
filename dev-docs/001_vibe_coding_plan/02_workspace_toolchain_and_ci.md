# 02. Workspace、工具链与 CI

## 1. 目标

建立可重复构建的 Rust/Web monorepo 和最小可执行服务，不加入业务假实现。

### BAS-003：初始化 Rust workspace

**前置：** BAS-001。  
**允许路径：** 根 Cargo/工具链配置、`apps/`、`crates/` 的 Phase 0 skeleton。

- [ ] 固定 Rust 1.96.1、Edition 2024、resolver 3，提交 `Cargo.lock`。
- [ ] 创建 `apps/security-platform`、`apps/migration-cli` 和 Phase 0 crates；每个 crate README 写职责、允许/禁止依赖。
- [ ] 根 workspace 统一依赖，启用 `unsafe_code=forbid`、`unwrap_used/expect_used/await_holding_lock=deny`。
- [ ] 二进制只完成配置加载、生命周期、健康服务和退出，不承载领域逻辑。
- [ ] 加入 license、repository、MSRV 和 aarch64 check。

**测试：** 空 workspace fmt/clippy/nextest；故意引入违规 lint 的 CI fixture 能失败。

### BAS-004：初始化 Web workspace

**前置：** BAS-001。  
**允许路径：** `web/`、根 Node 配置。

- [ ] 固定 Node 22.22.2、pnpm 11.12.0、React 19.2.8、TS 7.0.2、Vite 8.1.4、Semi 2.101.1。
- [ ] 创建 `web/apps/console`、`web/packages/api-client`、`web/packages/ui`、`web/packages/player`。
- [ ] 配置 strict TS、ESLint、Prettier、Vitest、Testing Library、Playwright 和 lockfile。
- [ ] 产物使用相对 asset manifest；禁止运行期从未配置第三方域加载代码。

**测试：** `pnpm typecheck && pnpm test && pnpm build`；连续构建产物清单稳定。

### BAS-005：CI 和依赖供应链

**前置：** BAS-003、BAS-004。

- [ ] PR jobs：fmt、clippy、nextest、frontend、migration、OpenAPI、Buf、deny/audit、secret scan。
- [ ] nightly jobs：PostgreSQL/NATS integration、浏览器矩阵、容器扫描、SBOM、aarch64。
- [ ] 缓存 key 包含工具链和 lockfile hash；生成任务在完成后检查 `git diff --exit-code`。
- [ ] Renovate/Dependabot 仅创建独立升级 PR，禁止自动合并 major。

**完成条件：** 故意破坏 Rust、TS、migration、许可证和生成 snapshot 时相应 job 必须失败。

## 2. 标准命令

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
cargo deny check
pnpm --dir web install --frozen-lockfile
pnpm --dir web typecheck
pnpm --dir web test
pnpm --dir web build
```

