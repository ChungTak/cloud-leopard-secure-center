# 07. 租户、组织与空间树

### ORG-001：Tenant 聚合
**前置：** DB-003。
- [x] 实现 code 不可变、Active/Suspended/Closed 状态和默认 locale/timezone。
- [x] 建立 `org.tenants`；平台级创建与状态操作必须走受审计管理上下文。
**测试：** code 唯一、终态、revision、Suspended 禁止新会话。

### ORG-002：组织树与 closure
**前置：** ORG-001。
- [ ] 建立 organization_units/closure，创建节点同时写 self row。
- [ ] move 在一个事务内验证非自身/非后代，更新 closure 并递增 revision。
- [ ] 删除前检查子节点和强依赖；列表稳定排序、分页有界。
**测试：** 根/深树、移动子树、循环、并发 move、事务中断和跨租户 parent。

### ORG-003：场所与空间树
**前置：** ORG-001。
- [ ] 建立 sites/buildings/floors/areas/area_closure 及同租户 FK。
- [ ] 组织表达管理关系，Area 表达物理位置；API/DTO 不复用一种 ID。
- [ ] geo 使用明确坐标系和合法范围；地址字段有长度限制。
**测试：** 唯一 code、层级约束、区域移动、删除引用和范围查询。

## 完成条件
树操作无需执行体自行拼 SQL；所有写路径复用领域服务与 UoW，closure 与实体永不部分提交。

