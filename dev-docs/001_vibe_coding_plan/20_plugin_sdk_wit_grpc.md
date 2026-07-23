# 20. 插件 SDK、WIT 与 gRPC

### PLG-001：Manifest、签名和生命周期
**前置：** AUD-002、MSG-001。
- [x] 新增 `crates/plugin-adapter` 与 `manifest` 模块；`PluginManifest` 固定 plugin_id、version、kind、api_range、capabilities/resources/events/config_digest。
- [x] `Plugin` aggregate 与生命周期状态机（Uploaded→Verified→Installed→Migrated→Enabled/Disabled/Quarantined），非法迁移返回 `Invalid`。
- [x] `ManifestVerifier` port 与 `UnsupportedManifestVerifier` stub；签名/Ed25519/checksum/SBOM/依赖/publisher trust 验证在真实实现中接入。
- [x] `foundation` 新增 `PluginId`。

### PLG-002：Wasm WIT host
**前置：** PLG-001。
- [x] `plugin-adapter/src/wit.rs` 定义 `WitHost` port，仅暴露 `log`、`read_config`、`query_resource`、`create_alarm`、`publish_event`。
- [x] `WasmLimits` 固定 fuel、memory_pages、max_calls、max_output_bytes、max_events、max_log_lines；`UnsupportedWitHost` 在已启用时返回 `Unsupported`。
- [x] `ResourceQuery`、`PluginEvent` 携带 tenant/plugin、causation 和 depth 字段；tenant/plugin scope 与能力再校验在真实 host 实现中接入。
**测试：** disabled/unconfigured 返回 `Unavailable`，enabled 返回 `Unsupported`，默认 limits 有限。

### PLG-003：进程插件 gRPC
**前置：** PLG-001、MSG-003。
- [ ] UDS/mTLS 双向 handshake：hello/welcome、版本、instance、scope、credit、heartbeat/config revision。
- [ ] frame 支持 command/result/event/health/drain/shutdown、seq/ack 和有限重放。
- [ ] 插件不获得 DB DSN、NATS 管理权限或 secret 枚举。

### PLG-004：Conformance kit
**前置：** PLG-002、PLG-003。
- [ ] 验证 manifest、签名、版本、资源、异常、幂等、权限、升级/回滚。
- [ ] 提供 Wasm 和进程示例插件；示例不得有生产假成功路径。

