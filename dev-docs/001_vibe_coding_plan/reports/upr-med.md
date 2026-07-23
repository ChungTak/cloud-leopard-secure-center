# UPR-MED cheetah-media-engine 上游需求报告

## 任务目标

锁定 cheetah-media-engine 必须发布的 Web SDK、资源清单、多画面与 source 切换契约。

## 变更摘要

- `dev-docs/001_vibe_coding_plan/91_cheetah_media_engine_upstream_requirements.md` 全部 `[x]` 标记并补充说明。
- 本仓 `web/packages/player` 已预留 `SecurityPlayer` 生命周期、`useSecurityPlayerWorker` stub、`playerConfig.ts` 的 `PlayerSecurityPolicy`/`securePlayerBrowserMatrix`。

## 验证

- `pnpm typecheck`/`pnpm test`/`pnpm lint` 通过。
- `loadSecurityPlayerWorker` 在 Worker/Wasm/codec 未实现时返回 `unsupported`。

## 未完成/降级

- 真实 `@cheetah-media/web` 包、Worker/Wasm/codec assets 与浏览器 contract fixtures 由上游发布；本仓不引入第二套播放器，仅做包装与降级 fallback。
