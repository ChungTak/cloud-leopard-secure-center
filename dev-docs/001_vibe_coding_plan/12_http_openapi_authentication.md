# 12. HTTP、OpenAPI 与认证

### API-001：OpenAPI 与错误中间件
**前置：** FND-002、APP-001。
- [x] OpenAPI 3.1 先定义 tenants/org/users/roles/bindings/devices/cameras/audit/settings。
- [x] 实现 RFC 9457、request/trace ID、body limit、timeout、CORS 和安全 header。
- [x] DTO/domain 显式 mapper；snapshot 和 breaking check 进入 CI。
**测试：** 400/401/403/404/409/412/422/429/503，不泄漏内部 source。

### API-002：认证与 tenant 边界
**前置：** IAM-003、API-001。
- [x] 验证 token 算法、签名、iss/aud/exp/nbf/jti/session_version；path tenant 必须匹配 scope。
- [x] handler 构造 RequestContext，repository 不从 header 自行推断 tenant。
- [x] 登录前/后独立限流；可信代理配置外不接受转发 IP。
**测试：** token 攻击矩阵、tenant path 伪造、代理 header、撤销即时生效。

### API-003：并发、分页和实时通知
**前置：** API-002、APP-002。
- [x] PUT/PATCH 使用 If-Match/ETag；写请求支持 Idempotency-Key。
- [x] cursor 不透明、可校验、稳定排序和最大 page size。
- [x] SSE 支持 filter/Last-Event-ID/有界 buffer；落后客户端收到 gap 后重查。
**测试：** ETag 冲突、游标篡改、重复 POST、慢 SSE、断线重连。

