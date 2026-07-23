# 08. RBAC 与资源范围

### AUTH-001：Permission 与 Role
**前置：** IAM-001、ORG-001。
- [x] 固定 permission key registry；未知 key 不能写入角色。
- [x] 实现 Role、role_permissions 和内置角色保护；租户角色不能授予平台权限。
- [x] 权限变更递增 role revision 并使缓存失效。
**测试：** 重名、未知权限、内置角色保护、跨租户 permission assignment。

### AUTH-002：RoleBinding 与 scope
**前置：** AUTH-001、ORG-002、ORG-003。
- [x] 支持 Tenant/OrganizationSubtree/AreaSubtree/ResourceSet，scope_ref 与类型匹配。
- [x] principal、role、scope 必须同租户；valid_from/until 使用 UTC。
- [x] ResourceSet 成员用 typed ResourceRef，数量和批量导入有上限。
**测试：** 过期绑定、组织/区域后代、资源集、移动树后权限变化。

### AUTH-003：AuthorizationPort
**前置：** AUTH-002。
- [ ] 输入 principal/tenant/action/resource/context；输出 allow/deny、policy IDs、安全 reason code。
- [ ] 默认拒绝；先验证 tenant，再解析角色和 scope；无 action 不 fallback 通配。
- [ ] 缓存 key 含 tenant/principal/action/resource revision；失效优先于 TTL。
**测试：** 表驱动允许/拒绝、缓存失效、敏感摄像机预留 context、P95 benchmark。

## 完成条件
业务模块只调用 AuthorizationPort，不直接查询 authz 表；前端权限不可替代服务端判断。

