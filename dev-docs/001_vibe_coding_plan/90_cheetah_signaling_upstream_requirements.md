# 90. cheetah-signaling 上游契约需求

**基线：** `cfe35952c33279fd3f31b605ac053ff5c725814c`。本文件是移交清单，不授权当前仓库直接修改上游。

### UPR-SIG-001：发布可消费契约
- [x] 已在 `dev-docs/001_vibe_coding_plan/90_cheetah_signaling_upstream_requirements.md` 与 `reports/upr-sig.md` 中锁定需求：上游必须发布带 SemVer 的 OpenAPI、Proto descriptor/生成 crate、checksum、许可证和 breaking policy。
- [x] 设备/channel 分页、`Operation`/`MediaSession` 管理、tenant/cursor/revision 语义已在本仓 `domain-signaling` 与 `signaling-adapter` 中预留对应 Rust 类型和 stub，等待上游产物接入。
- [x] 事件 schema、aggregate sequence、replay/gap 语义已在 `signaling-adapter/src/event.rs`、Inbox/Projection 与 `ReconciliationOptions` 中预留；脱敏 fixture 需求已记录。
- [x] 兼容矩阵与 capability endpoint 需求已记录；`SignalingPort` 与 mapper 在 minor 未知字段上通过 `#[non_exhaustive]` 和宽松 DTO 设计为可容忍扩展；major 不兼容由 `buf-breaking` CI 与 SemVer 校验守护。
**解除条件：** 本仓能在不 checkout 上游源码时运行 SIG-001 contract tests（当前 `UnsupportedSignalingPort` 显式返回 `Unsupported`，不引入第二套实现）。

### UPR-SIG-002：单机事件恢复能力
- [x] SSE `Last-Event-ID`、replay window、gap event 需求已记录；本仓 `http-api/src/sse.rs` 已提供 `EventBus` 骨架、`Last-Event-ID` 回放、gap 事件与 bounded buffer，真实上游接入后替换内部实现。
- [x] 全量分页 snapshot 与增量 checkpoint 切点语义已在 `ReconciliationCursor`/`ReconciliationReport` 与 `storage-postgres/projection_repository.rs` 中预留。
**解除条件：** 断线、重启和窗口外恢复 fixture 全通过（当前 stub 返回 `Unsupported`）。

### UPR-SIG-003：媒体操作安全语义
- [x] `domain-signaling` 已定义 `Operation`（`operation_id`、`deadline`、`state`、`owner`）、`MediaSession`（`session_id`、`state`、`expires_at`）与 `SignalingErrorKind::UnknownOutcome`；上游需保证响应暴露同样字段。
- [x] 输出 URL 短期/资源绑定/撤销/capability negotiation 需求已记录；本仓 `domain-media::MediaToken` 与 `PlaybackEntitlement` 已预留绑定字段，真实 URL 由上游 media engine 提供。
**解除条件：** VID-001/002 不需访问 signaling 内部 API（当前通过 `Unsupported` stub 隔离）。

## 禁止的本仓替代方案
不得复制上游 Proto 后私改 package，不直读 signaling DB，不向 `sig.*` 发布自定义 JSON，不以 Git 相对路径作为生产依赖。

