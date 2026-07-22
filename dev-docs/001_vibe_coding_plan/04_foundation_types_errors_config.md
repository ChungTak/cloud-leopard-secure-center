# 04. 基础类型、错误与配置

### FND-001：强类型标识、时间与并发

**前置：** ARC-001。  
**允许路径：** foundation 及其测试。

- [ ] 为 Tenant/User/Role/Organization/Area/Device/Camera/Binding/Audit/Message/Node 定义 UUIDv7 newtype。
- [ ] 定义 `Revision(u64)`、`UtcTimestamp`、`Deadline`、`ResourceRef`、不透明 `PageCursor`。
- [ ] 定义可注入 `Clock`、`IdGenerator`、`RandomSource`；domain 禁止直接取系统时间/随机数。
- [ ] 所有 parse 错误包含字段和稳定 code，不泄漏原输入中的 secret。

**测试：** 类型不可混用的 compile-fail；UUID/RFC3339/cursor round-trip；FakeClock 确定性测试。

### FND-002：统一错误与请求上下文

**前置：** FND-001。

- [ ] `PlatformError` 固定分类：invalid、unauthenticated、denied、not-found、exists、conflict、rate-limit、timeout、cancelled、unavailable、unsupported、version-mismatch、unknown-outcome、internal。
- [ ] 定义 `RequestContext`：request/correlation/trace、actor、tenant、deadline；禁止携带数据库连接或 HTTP extractor。
- [ ] adapter 映射 HTTP/gRPC/NATS 表示；domain 不携带 status code。
- [ ] 错误 source 仅用于内部 trace，公开 message 采用安全文本。

**测试：** 每类错误映射稳定；SQL/stack/secret 不进入序列化结果。

### FND-003：分层配置和 SecretPort

**前置：** FND-002。

- [ ] 配置顺序固定为默认值 → 文件 → 环境变量；未知字段拒绝启动。
- [ ] 定义 system/http/storage/security/observability/runtime 配置和单位明确的上下限。
- [ ] secret 只通过 `SecretProvider::resolve(SecretRef)` 获取；secret value 禁止 Debug/Serialize 并尽快 zeroize。
- [ ] 配置分 static/dynamic；static 变化要求重启，dynamic 通过版本化快照应用。
- [ ] 提供 `config.example.toml`，只含假值和注释，不提供不安全生产默认。

**测试：** precedence、未知字段、边界、缺 secret、脱敏和热更新失败保持旧配置。

## 完成条件

foundation 无 runtime/framework 依赖，公共类型均有 rustdoc、属性测试和稳定序列化规范。

