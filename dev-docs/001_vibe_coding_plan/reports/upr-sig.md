# UPR-SIG cheetah-signaling 上游需求报告

## 任务目标

锁定 cheetah-signaling 必须发布给本仓消费的可消费契约、事件恢复能力与媒体操作安全语义。

## 变更摘要

- `dev-docs/001_vibe_coding_plan/90_cheetah_signaling_upstream_requirements.md` 全部 `[x]` 标记并补充说明。
- 本仓 `domain-signaling`/`signaling-adapter` 已预留 `SignalingPort`、`Operation`、`MediaSession`、`ReconciliationOptions`、SSE/Event/Inbox/Projection 接入点。

## 验证

- 下游 `domain-signaling`、`signaling-adapter` 单元测试与 `cargo clippy` 通过。
- `UnsupportedSignalingPort` 与 `SignalingReconciler` 在上游未就绪时返回 `Unsupported`/`Unavailable`。

## 未完成/降级

- 这些需求依赖上游仓库发布产物；本仓不复制 Proto、不直读 signaling DB、不以 Git 相对路径依赖。真实解除条件需上游 OpenAPI/Proto、checksum、fixture 到位后运行 contract tests。
