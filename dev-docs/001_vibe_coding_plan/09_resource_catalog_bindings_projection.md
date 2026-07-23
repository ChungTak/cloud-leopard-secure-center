# 09. 资源目录、外部绑定与投影

### RES-001：ManagedDevice 与 Camera
**前置：** ORG-003、AUTH-003、DB-003。
- [x] 实现 Draft/Active/Disabled/Retired，区分业务 lifecycle 与外部 online state。
- [x] 建立 managed_devices/cameras，组织和区域引用同租户；Camera sensitivity 独立建模。
- [x] serial 不是主键；设备退休不级联删除历史审计。
**测试：** 生命周期、引用、revision、查询 scope 和敏感资源授权。

### RES-002：Tag 与 ExternalBinding
**前置：** RES-001。
- [x] Tag key/value 规范化和数量限制；资源类型来自 registry。
- [x] ExternalBinding 状态 Pending/Active/Stale/Conflict/Disabled；外部 ref 有效时全局唯一。
- [x] 自动匹配只能创建 Pending，激活需可信规则或人工操作并审计。
**测试：** 双绑定冲突、并发激活、上游 ID 不同类型、解绑后历史保留。

### RES-003：Signaling 投影表
**前置：** RES-002。
- [ ] 建立 device/channel projection、checkpoint、failure 表；写权限仅 projection worker。
- [ ] API 总是返回 observed_at/source_event_id/stale；投影不能覆盖平台资产字段。
- [ ] 支持 shadow rebuild 和原子切换读视图。
**测试：** 重复/乱序/缺口/重建、同序列不同 payload quarantine、过期标记。

## 完成条件
平台数据库中不存在 signaling owner、protocol session、Operation、MediaSession 或 MediaBinding 的竞争权威表。

