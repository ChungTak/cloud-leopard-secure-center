# 16. cheetah-signaling 契约与资源投影

### SIG-001：冻结上游产物和 SignalingPort
**前置：** PKG-002、UPR-SIG-001。
- [ ] 锁定 OpenAPI/Proto descriptor、版本、checksum 和许可证；禁止跨仓相对路径。
- [ ] 定义 get-device/create-operation/create-media-session/get-operation port、typed IDs、deadline 和错误映射。
- [ ] REST DTO、cheetah Proto 与平台 snapshot 显式 mapper；Unsupported/Unavailable/UnknownOutcome 分开。
**测试：** fake 与录制契约 fixture、未知 enum/字段、超时/取消、敏感字段脱敏。

### SIG-002：REST + SSE 单机 adapter
**前置：** SIG-001、RES-003。
- [ ] REST client 使用 rustls、连接池、每操作 deadline；只调用公开 API。
- [ ] SSE 支持 Last-Event-ID、重连上限、gap；事件先进入 Inbox 再更新投影/checkpoint。
- [ ] signaling 不可用只使投影 stale，不影响 IAM/组织管理。
**测试：** 重复、乱序、gap、断线、慢流、服务重启和全量重建。

### SIG-003：全量 reconciliation
**前置：** SIG-002。
- [ ] 分页拉取 device/channel，写 shadow projection；每页有 cursor/checksum/limit。
- [ ] 完整校验后原子切换；上游缺失先标 missing，保留窗口后清理。
- [ ] 增量事件在 rebuild 期间有界缓存或从明确 checkpoint 重放。
**测试：** 百万级模拟分页、中断恢复、切换失败、事件与 rebuild 竞争。

集群 JetStream 投影由 Phase 3 的 `SIG-004` 实现；Phase 2 只以 REST + SSE 完成可独立验收的单机闭环。
