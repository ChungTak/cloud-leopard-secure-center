# 90. cheetah-signaling 上游契约需求

**基线：** `cfe35952c33279fd3f31b605ac053ff5c725814c`。本文件是移交清单，不授权当前仓库直接修改上游。

### UPR-SIG-001：发布可消费契约
- [ ] 发布带 SemVer 的 OpenAPI、Proto descriptor/生成 crate、checksum、许可证和 breaking policy。
- [ ] 公开设备、channel 分页查询和 Operation/MediaSession 管理契约；tenant、cursor、revision 语义一致。
- [ ] 提供事件 schema/type registry、aggregate sequence、replay/gap 说明和脱敏 fixture。
- [ ] 发布兼容矩阵和 capability endpoint；未知 minor 字段可容忍，major 不兼容明确拒绝。
**解除条件：** 本仓能在不 checkout 上游源码时运行 SIG-001 contract tests。

### UPR-SIG-002：单机事件恢复能力
- [ ] SSE 支持 Last-Event-ID、明确 replay window 和 gap event，或提供等价签名 Webhook。
- [ ] 全量分页 snapshot 与增量 checkpoint 有一致切点/恢复说明。
**解除条件：** 断线、重启和窗口外恢复 fixture 全通过。

### UPR-SIG-003：媒体操作安全语义
- [ ] Operation/MediaSession 响应暴露稳定 ID、deadline、owner epoch 诊断和 UnknownOutcome。
- [ ] 输出 URL 服务支持短期、资源绑定、撤销和 capability negotiation。
**解除条件：** VID-001/002 不需访问 signaling 内部 API。

## 禁止的本仓替代方案
不得复制上游 Proto 后私改 package，不直读 signaling DB，不向 `sig.*` 发布自定义 JSON，不以 Git 相对路径作为生产依赖。

