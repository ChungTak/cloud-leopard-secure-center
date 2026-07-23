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
- [x] `plugin-adapter/src/grpc.rs` 定义 `PluginHello`（version/instance/scope/credits）、`HostWelcome`（heartbeat/config_revision/allowed_capabilities）与 `PluginFrame`（Command/Result/Event/Health/Drain/Shutdown，含 seq/ack）。
- [x] `ProcessPluginHost` port 与 `UnsupportedProcessPluginHost` stub；未配置返回 `Unavailable`，已启用返回 `Unsupported`。
- [x] 握手 scope 与 allowed_capabilities 用于后续能力裁剪；插件不暴露 DSN/NATS/secret 访问。

### PLG-004：Conformance kit
**前置：** PLG-002、PLG-003。
- [x] 新增 `plugin-adapter/tests/conformance.rs` 覆盖 manifest 字段、生命周期非法迁移、quarantine 可达性、`UnsupportedManifestVerifier`、Wit/Process host `Unsupported`、frame JSON 往返。
- [x] 真实签名/版本/资源/异常/幂等/权限/升级回滚与 Wasm/进程示例插件在真实 host 实现后补齐；当前测试只验证 contract，无假成功路径。

