# 06. 身份、凭据与会话

### IAM-001：User 聚合与仓储
**前置：** DB-003。  
- [x] 实现 User 的 Pending/Active/Locked/Disabled 状态、username 规范化、session_version 和 revision。
- [x] 建立 `iam.users/user_identities` DDL、tenant 唯一索引和 repository。
- [x] 状态变化生成领域事件，不直接发布。
**测试：** 重名、非法迁移、并发更新、跨租户、soft delete 后重建。

### IAM-002：密码与登录
**前置：** IAM-001、FND-003。
- [x] 使用 Argon2id；hash 记录算法参数，登录时按新策略重算。
- [x] 凭据表仅 IAM 账号可读；日志/错误不含 username 原值之外的凭据数据。
- [x] 登录失败按来源和账号双限流，达到策略后锁定并审计。
**测试：** 正确/错误密码、时序安全、旧 hash 升级、锁定、并发登录。

### IAM-003：Token 与 refresh rotation
**前置：** IAM-002。
- [ ] access token 只含 subject、tenant、session/version、aud/iss/exp/jti。
- [ ] refresh token 只存 hash；每次刷新轮换，同 family 重用触发全族撤销。
- [ ] 用户禁用、密码修改和显式退出递增 session_version/撤销 session。
**测试：** 过期、错误 issuer/audience、旧版本、refresh replay、并发 refresh 仅一方成功。

### IAM-004：MFA、服务账号与 API key
**前置：** IAM-003。
- [ ] MFA 只存 secret ref，恢复码哈希保存；定义 assurance level。
- [ ] 服务账号/API key 有 scope、来源限制、过期和撤销；key 仅创建时返回一次。
- [ ] 高风险 action 接收最小 assurance 要求。
**测试：** MFA replay、过期 key、scope 越界、密钥不进入审计 details。

## 完成条件
所有认证失败返回一致稳定错误，且不会泄漏账号是否存在；登录、刷新、撤销和 key 管理均有审计。

