# 19. 告警、通知与联动

### ALM-001：告警聚合与存储
**前置：** MSG-002、AUTH-003。
- [ ] 状态 NEW→ACKNOWLEDGED→PROCESSING→RESOLVED→CLOSED，显式 SUPPRESSED/MERGED/REOPENED。
- [ ] 建立 alarm/alarm_events/assignments/actions/evidence；证据存对象引用和 checksum。
- [ ] dedup key、aggregation window、severity 和 tenant policy 均有上限。
**测试：** 全迁移表、重复事件、并发处置、越权、证据损坏。

### ALM-002：通知
**前置：** ALM-001、APP-003。
- [ ] 站内/SSE/webhook 先实现统一 NotificationPort；模板变量白名单。
- [ ] delivery 有幂等、deadline、退避、熔断、DLQ；Webhook 防 SSRF/DNS rebinding。
- [ ] 通知失败不回滚告警权威状态。

### ALM-003：联动工作流
**前置：** ALM-002、PLG-002。
- [ ] 条件、动作、冷却、最大派生深度和 loop detection 显式配置。
- [ ] 高风险设备动作再次授权；无法确认结果为 UNKNOWN_OUTCOME。
- [ ] 每步写 workflow attempt 和审计，可重放但不重复副作用。

