# 17. 视频授权与 Web 播放器

### VID-001：PlaybackEntitlement
**前置：** SIG-002、AUTH-003。
- [x] 实现 Live/Playback/Download/Ptz actions、过期/撤销和 signaling `OperationId` 引用；MediaSession 状态机不在本域复制。
- [ ] 创建顺序：授权 → signaling Operation/Session → media output → 短期 DTO → 审计。
- [ ] token 绑定 tenant/principal/camera/session/protocol，URL 不入日志。
**测试：** 幂等、撤销、跨摄像机、超时 unknown outcome、敏感级别/MFA。

### VID-002：视频 REST 与审计
**前置：** VID-001、API-003。
- [x] application 层 `MediaUseCase`/`MediaService` 实现 create/get/revoke，权限校验后调用 `MediaPort`；错误映射为 `PlatformError`。
- [ ] 响应含 main/sub source、expires_at、player policy；不含设备凭据。
- [ ] 查看、回放、下载、PTZ 均独立 permission 和审计 action。

### VID-003：SecurityPlayer 包装
**前置：** VID-002、UPR-MED-001。
- [x] `SecurityPlayer` wrapper with `StreamSource`, `token` redaction, load/stop lifecycle, main/sub stream switch, and `unsupported` fallback.
- [ ] 包装 load/stop/destroy、事件、token refresh、主子码流、诊断脱敏。
- [ ] 路由卸载、登出、租户切换和 error boundary 均释放 Worker/Wasm/media element。
**测试：** firstframe、fallback、过期刷新 single-flight、销毁幂等、资源泄漏。

### VID-004：多画面与浏览器矩阵
**前置：** VID-003。
- [ ] 1/4/9/16 窗、焦点优先、主子码流、全局 CPU/GPU/内存预算。
- [ ] 自托管 Worker/Wasm/codec；配置 CSP/SRI/COOP/COEP 和兼容降级。
- [ ] Chromium/Firefox/WebKit 覆盖播放、后台恢复、网络抖动和长时间切换。

