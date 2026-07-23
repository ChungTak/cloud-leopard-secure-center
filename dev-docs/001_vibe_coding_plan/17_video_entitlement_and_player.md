# 17. 视频授权与 Web 播放器

### VID-001：PlaybackEntitlement
**前置：** SIG-002、AUTH-003。
- [x] 实现 Live/Playback/Download/Ptz actions、过期/撤销和 signaling `OperationId` 引用；MediaSession 状态机不在本域复制。
- [x] `application/src/media.rs` 创建顺序：鉴权（`media:entitlement:create`）→ `MediaPort::create_entitlement` → `PlaybackEntitlementDto`；审计 action 与 signaling operation 由下游 port 接入。
- [x] `domain-media` 新增 `MediaSession`、`MediaToken` 与 `PlaybackEntitlement.token`；`MediaToken` 绑定 `tenant_id`/`principal_id`/`camera_id`/`session_id`/`protocol`/`expires_at`；视频 URL 仅在 `StreamSource` DTO 中传递，审计与日志记录中不写入 URL。
**测试：** 幂等、撤销、跨摄像机、超时 unknown outcome、敏感级别/MFA。

### VID-002：视频 REST 与审计
**前置：** VID-001、API-003。
- [x] application 层 `MediaUseCase`/`MediaService` 实现 create/get/revoke，权限校验后调用 `MediaPort`；错误映射为 `PlatformError`。
- [x] `PlaybackEntitlementDto` 返回 `main_source`/`sub_source`/`expires_at`/`player_policy`，不含设备凭据；`PlayerPolicy` 包含 `autoplay`/`controls`/`muted`/`allowed_actions`。
- [x] `MediaService` 分别为 create/get/revoke 使用 `media:entitlement:create`/`media:entitlement:read`/`media:entitlement:revoke` 权限，并为每种 action 留下独立审计点。

### VID-003：SecurityPlayer 包装
**前置：** VID-002、UPR-MED-001。
- [x] `SecurityPlayer` wrapper with `StreamSource`, `token` redaction, load/stop lifecycle, main/sub stream switch, and `unsupported` fallback.
- [x] `SecurityPlayer` 暴露 `onLoad`/`onDestroy`/`onTokenExpired` 生命周期与 `onDiagnostics`（URL 已脱敏）；`token` 不渲染、不写入日志；`useEffect` cleanup 中调用 `playerRef.stop()` 与 `onDestroy()`，在组件卸载/登出/租户切换时释放资源。
- [x] `tokenRefreshUrl` 与 `onTokenExpired` 提供刷新扩展点；主子码流切换通过 `handleSwitchStream` 完成；`redactUrl` 对 token 查询参数脱敏。
**测试：** firstframe、fallback、过期刷新 single-flight、销毁幂等、资源泄漏。

### VID-004：多画面与浏览器矩阵
**前置：** VID-003。
- [x] `MultiPlayerLayout` 支持 1/4/9/16 窗格与自定义 slot renderer。
- [x] `web/packages/player/src/useSecurityPlayerWorker.ts` 提供 `loadSecurityPlayerWorker` stub；真实 Worker/Wasm/codec 未实现时返回 `{ ok: false, error: 'unsupported' }`，组件可降级到 native `<video>`。
- [x] `playerConfig.ts` 定义 `PlayerSecurityPolicy`、`defaultPlayerSecurityPolicy` 与 `securePlayerBrowserMatrix`（Chromium/Firefox/WebKit/Edge），由宿主应用/反向代理在真实部署时注入 CSP/SRI/COOP/COEP 头。
- [x] 长时间切换、后台恢复、网络抖动等场景依赖上游 media engine，当前通过 `unsupported` fallback 处理。

