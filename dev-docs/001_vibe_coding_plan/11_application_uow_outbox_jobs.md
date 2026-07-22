# 11. 应用服务、UoW、Outbox/Inbox 与任务

### APP-001：应用用例模板
**前置：** IAM-001、AUTH-003、DB-003。
- [ ] 固定顺序：context/tenant → validation → authorization → load → domain method → UoW save/outbox/audit → DTO。
- [ ] 为 Tenant/User/Organization/Role/Device/Config 用例实现 ports，不接触具体 SQL。
- [ ] 所有写用例接受 expected revision 和 idempotency context。
**测试：** fake ports 覆盖成功、拒绝、冲突、取消、审计失败。

### APP-002：Idempotency 与 Outbox
**前置：** APP-001。
- [ ] idempotency 唯一键为 tenant/principal/endpoint/key；保存 request digest 和首次响应。
- [ ] 相同 key 不同 digest 返回 `IDEMPOTENCY_CONFLICT`。
- [ ] 聚合、审计和 outbox 在允许的 UoW 内原子提交；publisher 使用 claim/lease 和有界 batch。
**测试：** 100 次重复只产生一次逻辑变更；commit/publish crash window；publisher 双实例。

### APP-003：Inbox 与后台任务
**前置：** APP-002。
- [ ] Inbox 按 consumer/message 去重并保存首次结果 digest；保留期覆盖最大重放窗口。
- [ ] Job 使用 status/revision/lease_owner/lease_until/attempts/next_run；旧 lease 结果被拒绝。
- [ ] 重试仅对分类 transient error，指数退避+jitter+总 deadline。
**测试：** ack 丢失、consumer restart、poison、lease 过期、取消、时钟推进。

