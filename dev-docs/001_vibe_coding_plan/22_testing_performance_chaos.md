# 22. 测试、性能与混沌

### TST-002：统一测试基础设施
**前置：** MSG-004、PLG-004。
- [ ] 提供 FakeClock/IDs/Secret/Bus/Signaling、真实 PostgreSQL/NATS 容器和 tenant fixture。
- [ ] 测试端口由 OS 分配；fixture 有来源、许可、脱敏和预期 manifest。
- [ ] contract suite 可运行 fake/real、local/NATS、SSE/JetStream adapter。

### TST-003：性能基线
**前置：** TST-002、VID-004。
- [ ] 固定数据生成器、请求组合、硬件、配置和持续时间。
- [ ] 验证 100 tenant、10万用户/设备、20万摄像机、1000并发用户及设计 P95。
- [ ] 单独报告 DB、授权、Outbox、投影、播放器；阈值回退使 CI/nightly 失败。

### TST-004：故障与长期稳定性
**前置：** TST-003、OBS-002。
- [ ] 注入 PostgreSQL failover、NATS 节点/网络分区、signaling/media/plugin 崩溃、磁盘满、时钟偏移。
- [ ] 验证无跨租户、无重复危险副作用、旧 epoch 被拒绝、积压可恢复。
- [ ] 运行 72h soak，报告内存/连接/task/lag 趋势和恢复时间。

