# 05. PostgreSQL、迁移与 RLS

## 1. Schema 与连接规则

固定 schema：`iam`、`org`、`authz`、`resource`、`audit`、`config`、`infra`，后续加入 `alarm`、`plugin`。运行账号不拥有 schema；migration 使用独立高权限账号。

### DB-001：迁移框架和通用 DDL

**前置：** FND-001、FND-003。

- [x] 建立 append-only SQLx migrations 和 `migration-cli`。
- [x] 创建 schema、数据库角色、`infra.schema_metadata` 和必要扩展；不依赖非标准扩展实现核心树。
- [x] 通用权威表使用 UUID、tenant、revision、UTC 时间、actor 和 soft delete 列。
- [x] 状态使用稳定 text + CHECK；JSONB 必须有 schema version 和应用大小限制。
- [x] migration 名称标明 expand/backfill/switch/contract；启动只执行短时兼容阶段。

**测试：** 空库建库、重复执行、上一版本升级、失败回滚；已发布 migration checksum 不变。

### DB-002：RLS 和 tenant transaction

**前置：** DB-001。

- [x] 所有租户表 ENABLE + FORCE RLS，policy 同时约束 USING/WITH CHECK。
- [x] storage adapter 每个 tenant 操作开启 transaction 后执行 `SET LOCAL app.tenant_id`。
- [x] pool 归还连接后 tenant context 不残留；无上下文默认拒绝。
- [x] 平台管理使用独立受限角色和显式 API，不通过隐藏 bypass flag。

**测试：** 两租户读写/关联/分页/并发；伪造 path、遗漏 context、pool 重用均不能串读。

### DB-003：Repository contract 与乐观锁

**前置：** DB-002、FND-002。

- [x] storage-api 按聚合定义最小 repository，不暴露 row、SQL error 或通用 transaction closure。
- [x] update/delete 必须带 expected revision；零行映射 `REVISION_CONFLICT` 或 `NOT_FOUND`，不可混淆。
- [x] 定义明确 UnitOfWork 组合，聚合与 Outbox 同事务。
- [x] cursor 包含稳定排序键并签名/校验，设置最大 page size。

**测试：** CRUD、软删除唯一性、并发写、事务回滚、游标篡改、RLS 和连接中断。

### DB-004：分区、备份和维护

**前置：** DB-001。

- [x] audit/login-attempt 按月分区并预建未来分区；默认分区只用于告警，不长期承载数据。
- [x] 清理使用有界 batch 和可恢复 checkpoint。
- [x] 提供 pg_dump/restore 验证脚本和分区 runbook。
- [x] CI 使用 PostgreSQL 17；兼容流水线覆盖下一受支持 major。

## 完成条件

所有 repository 通过同一真实 PostgreSQL contract suite；不存在 SQLite adapter 或测试替代生产 RLS 的内存假结论。
