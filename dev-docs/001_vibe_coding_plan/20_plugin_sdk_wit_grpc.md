# 20. 插件 SDK、WIT 与 gRPC

### PLG-001：Manifest、签名和生命周期
**前置：** AUD-002、MSG-001。
- [ ] manifest 固定 plugin/version/kind/API range/capabilities/resources/events/config digest。
- [ ] Ed25519 签名、checksum、SBOM、依赖和 publisher trust 验证先于安装。
- [ ] 状态 Uploaded/Verified/Installed/Migrated/Enabled/Disabled/Quarantined；非法迁移失败。

### PLG-002：Wasm WIT host
**前置：** PLG-001。
- [ ] WIT v1 只暴露 log/read-config/query-resource/create-alarm/publish-event。
- [ ] 默认无 filesystem/network/database/secret；每次执行限制 fuel、epoch deadline、memory、calls/output/events/logs。
- [ ] host capability 再校验 tenant/plugin scope；派生事件携带 causation/depth。
**测试：** 无限循环、内存爆、越权 host call、事件风暴、坏 component。

### PLG-003：进程插件 gRPC
**前置：** PLG-001、MSG-003。
- [ ] UDS/mTLS 双向 handshake：hello/welcome、版本、instance、scope、credit、heartbeat/config revision。
- [ ] frame 支持 command/result/event/health/drain/shutdown、seq/ack 和有限重放。
- [ ] 插件不获得 DB DSN、NATS 管理权限或 secret 枚举。

### PLG-004：Conformance kit
**前置：** PLG-002、PLG-003。
- [ ] 验证 manifest、签名、版本、资源、异常、幂等、权限、升级/回滚。
- [ ] 提供 Wasm 和进程示例插件；示例不得有生产假成功路径。

