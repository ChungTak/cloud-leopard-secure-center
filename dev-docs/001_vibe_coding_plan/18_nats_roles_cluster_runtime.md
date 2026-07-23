# 18. NATS、角色与集群运行时

### MSG-001：MessageBus contract 与 local adapter
**前置：** APP-003。
- [x] `message-api` 定义 `Envelope`/`CommandEnvelope`/`EventEnvelope`、`MessageError`、`MessageBus` port，payload 为 opaque bytes，支持 JSON 编解码与 deadline。
- [x] `message-local` 实现内存 `LocalMessageBus`：ack/nack、max-nack dead-letter、`max_in_flight` 背压、简单 topic wildcard、不持久化。
- [x] 新增 `proto/security/v1/messages.proto`（enum 0=UNSPECIFIED、reserved 字段/编号）与 `proto/buf.yaml`、`.github/workflows/buf-breaking.yml`；本地 `buf` 未安装时标记 `UNSUPPORTED`。

### MSG-002：NATS Core/JetStream adapter
**前置：** MSG-001。
- [x] 新增 `crates/nats-adapter`，实现 `MessageBus` trait；固定 streams 名称 `SECURITY_COMMANDS`/`SECURITY_EVENTS`、durable name `security-platform`、max deliver 3。
- [x] 无 `servers` 配置时返回 `Unavailable`；有配置时返回 `Unsupported`（尚未接入 `async-nats`），保证 subject/ACL/TLS/deadline 约束留到后续实现。
**测试：** `unconfigured_nats_returns_unavailable`、`configured_nats_returns_unsupported`。

### SIG-004：JetStream 投影 adapter
**前置：** MSG-002、SIG-001。
- [ ] 消费 `sig.v1.event.{bucket}.{type}`，不创建或修改上游 stream。
- [ ] durable 名称、ACL、ack/nak/term、dead-letter 和 replay 固定配置。
- [ ] SSE/JetStream 通过同一 projection contract suite。
- [ ] 集群切换传输不重置业务 projection checkpoint 或制造双消费。
**测试：** 与 SIG-002 相同的重复、乱序、gap、重放和 stale 场景。

### MSG-003：节点租约和角色调度
**前置：** ARC-003、MSG-002。
- [ ] KV buckets 为 SECURITY_NODES/CAPABILITIES；descriptor 含 role/zone/build/capacity/contracts。
- [ ] CAS lease、instance epoch、drain 和过期不可调度；旧 epoch 结果 fenced。
- [ ] workflow/scheduler 单任务 lease 与 DB revision 双保护。

### MSG-004：集群装配
**前置：** MSG-003、SIG-004。
- [ ] api/workflow/projection/scheduler/plugin-host 独立运行与 `all` 行为一致。
- [ ] readiness 按角色依赖；滚动关闭先 drain，再停 consumer/listener。
- [ ] Local/NATS、单/多实例通过同一 use-case contract。
