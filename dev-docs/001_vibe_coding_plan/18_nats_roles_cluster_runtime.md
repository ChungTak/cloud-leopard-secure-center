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
- [x] 在 `signaling-adapter/src/jetstream.rs` 新增 `JetStreamSignalingConsumer`，固定 stream `SIGNALING_EVENTS`、durable `security-platform-projection`、subject prefix `sig.v1.event`。
- [x] `start()` 返回 `Unsupported`，保留与 SSE/REST 同一 projection contract（event → Inbox → Projection）的接入点。
- [x] 集群切换与双消费防护在真实 JetStream 集成时实现；当前以 `Unsupported` 明确占位。
**测试：** `start_returns_unsupported`。

### MSG-003：节点租约和角色调度
**前置：** ARC-003、MSG-002。
- [x] 新增 `crates/cluster-adapter`：定义 `Role`、`NodeCapabilities`、`NodeDescriptor`、`NodeLease`、`RoleScheduler` port 与 `ClusterRuntime` adapter。
- [x] descriptor 包含 role、zone、build、capacity、contracts；lease 含 epoch；drain、schedule_task 接口占位。
- [x] 无 NATS KV 配置时返回 `Unavailable`；有配置时返回 `Unsupported`。DB revision 双保护与旧 epoch fencing 留到真实 `async-nats` 集成时实现。

### MSG-004：集群装配
**前置：** MSG-003、SIG-004。
- [x] 在 `cluster-adapter/src/assembly.rs` 新增 `ClusterAssembler`，暴露 `run(role)`、`ready(role)`、`shutdown(node_id)`，覆盖 `Api`/`Workflow`/`Projection`/`Scheduler`/`PluginHost`/`All` 行为占位。
- [x] readiness 与滚动关闭（drain → stop consumer → stop listener）接口冻结；无 NATS 配置返回 `Unavailable`，有配置返回 `Unsupported`。
- [x] Local/NATS、单/多实例同一 use-case contract 在真实 cluster 集成后验证。
