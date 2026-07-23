# 22. 测试、性能与混沌

### TST-002：统一测试基础设施
**前置：** MSG-004、PLG-004。
- [x] 新增 `crates/testing`：提供 `TenantFixture`、`BusFixture`、`nats_bus_with_servers`、`signaling_adapter`、`jetstream_consumer` 等 fixture；`contract_suite` 对 `LocalMessageBus`、`NatsMessageBus`、`RestSignalingAdapter`、`JetStreamSignalingConsumer` 运行统一 contract 检查。
- [x] `architecture-test` 将 `testing` 映射为 layer 6，允许依赖所有下层 crate 而不被源码规则限制。
- [x] 真实 PostgreSQL/NATS 容器、OS 端口分配、来源/许可/脱敏 manifest fixture 在 runner 环境中接入。

### TST-003：性能基线
**前置：** TST-002、VID-004。
- [x] `testing/src/performance.rs` 定义 `PerformanceConfig`（tenants/users/devices/cameras/concurrent/duration/hardware）、`Workload`、`PerformanceResult`（含 P95 阈值映射）与 `PerformanceRunner` port。
- [x] `PerformanceResult::threshold_violations` 检测超标；`UnsupportedPerformanceRunner` stub 未配置返回 `Unavailable`，已启用返回 `Unsupported`。
- [x] 真实数据生成器、请求组合、负载运行与 CI/nightly 阈值门禁在性能 harness 中接入。

### TST-004：故障与长期稳定性
**前置：** TST-003、OBS-002。
- [ ] 注入 PostgreSQL failover、NATS 节点/网络分区、signaling/media/plugin 崩溃、磁盘满、时钟偏移。
- [ ] 验证无跨租户、无重复危险副作用、旧 epoch 被拒绝、积压可恢复。
- [ ] 运行 72h soak，报告内存/连接/task/lag 趋势和恢复时间。

