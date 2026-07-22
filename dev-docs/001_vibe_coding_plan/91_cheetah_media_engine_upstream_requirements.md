# 91. cheetah-media-engine 上游契约需求

**基线：** `49531f6f863840e7c4211bd66917c9711abf3305`。本仓只消费发布产物。

### UPR-MED-001：发布 Web SDK 与资源清单
- [ ] 发布固定版本的 `@cheetah-media/web`、`@cheetah-media/components`、类型声明、Worker/Wasm/codec assets。
- [ ] manifest 包含内容 hash、ABI、浏览器要求、CSP/SRI/COOP/COEP 和自托管说明。
- [ ] API 明确 load/stop/destroy 幂等、事件顺序、错误 code/stage/recoverable、URL/header 脱敏。
- [ ] 提供 Chromium/Firefox/WebKit contract fixture 和资源泄漏测试。
**解除条件：** VID-003 可只用 registry/tarball 安装并完成离线构建。

### UPR-MED-002：多画面与 source 切换契约
- [ ] 稳定 1/4/9/16 wall API、全局预算、主子流原子切换和失败保留旧源语义。
- [ ] 暴露 firstframe/backendchange/buffering/stats，频率和内存历史有上限。
**解除条件：** VID-004 无需访问 package 内部 runtime 对象。

## 本仓适配原则
平台包装生命周期、业务 token 和审计，不复制 demux/decoder/render；上游缺失时返回明确 Unsupported，不引入第二套播放器。
