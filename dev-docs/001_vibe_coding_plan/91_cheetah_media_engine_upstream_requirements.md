# 91. cheetah-media-engine 上游契约需求

**基线：** `49531f6f863840e7c4211bd66917c9711abf3305`。本仓只消费发布产物。

### UPR-MED-001：发布 Web SDK 与资源清单
- [x] 需求已锁定：上游需发布固定版本的 `@cheetah-media/web`、`@cheetah-media/components`、类型声明、Worker/Wasm/codec assets。
- [x] manifest 字段需求（内容 hash、ABI、浏览器要求、CSP/SRI/COOP/COEP、自托管说明）已记录到 `playerConfig.ts` 的 `PlayerSecurityPolicy` 与 `securePlayerBrowserMatrix`。
- [x] API 语义（load/stop/destroy 幂等、事件顺序、错误 code/stage/recoverable、URL/header 脱敏）已记录到 `SecurityPlayer` 接口与 `useSecurityPlayerWorker.ts` stub。
- [x] Chromium/Firefox/WebKit 兼容矩阵已定义；contract fixture 与资源泄漏测试依赖上游产物，当前由 `unsupported` fallback 替代。
**解除条件：** VID-003 可只用 registry/tarball 安装并完成离线构建（当前 `loadSecurityPlayerWorker` 返回 `{ ok: false, error: 'unsupported' }`）。

### UPR-MED-002：多画面与 source 切换契约
- [x] `MultiPlayerLayout` 已支持 1/4/9/16 窗格；主子流切换由 `SecurityPlayer` 的 `handleSwitchStream` 处理；失败保留旧源语义由 onError 不更新 `activeUrl` 与 fallback 逻辑预留。
- [x] 事件类型（firstframe/backendchange/buffering/stats）与诊断脱敏已在 `SecurityPlayer` 的 `onFirstFrame`/`onDiagnostics`/redactUrl 中预留；真实频率/内存历史上限由上游 SDK 实现。
**解除条件：** VID-004 无需访问 package 内部 runtime 对象（当前以 stub fallback 运行）。

## 本仓适配原则
平台包装生命周期、业务 token 和审计，不复制 demux/decoder/render；上游缺失时返回明确 Unsupported，不引入第二套播放器。
