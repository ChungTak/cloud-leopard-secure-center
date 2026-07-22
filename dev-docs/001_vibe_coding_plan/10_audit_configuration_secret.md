# 10. 审计、配置与密钥

### AUD-001：追加写审计
**前置：** DB-004、FND-002。
- [ ] 建立月分区 audit.records 和专用 writer；业务账号无 UPDATE/DELETE。
- [ ] 记录 actor、tenant、action、target、result、request/trace、IP、前后 digest；details 有 schema/大小限制。
- [ ] 成功与拒绝的高风险操作均审计；审计失败按 action 风险选择拒绝或告警，不静默丢失。
**测试：** 修改/删除被 DB 拒绝、分区路由、脱敏、审计 writer 故障策略。

### AUD-002：配置定义和值
**前置：** FND-003、DB-003。
- [ ] Definition 固定类型/schema/default/sensitive/dynamic；Value 按 platform/tenant/module scope 唯一。
- [ ] 解析优先级明确；非法新值不替换旧快照。
- [ ] sensitive definition 只允许 secret_ref，API 永不返回 resolved value。
**测试：** schema、scope、revision、动态 reload、secret redaction。

### AUD-003：保留与清理
**前置：** AUD-001。
- [ ] 为 audit/login/outbox/inbox 定义默认保留和 tenant override 边界。
- [ ] 清理使用租约、cursor 和有界 batch；legal hold 资源禁止删除。
- [ ] 分区 drop 前生成审计和可恢复备份确认。
**测试：** 中断恢复、双 worker、hold、磁盘接近上限。

