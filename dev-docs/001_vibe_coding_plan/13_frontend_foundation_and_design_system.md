# 13. 前端基础与 Design System

### WEB-001：应用壳与 Semi 主题
**前置：** BAS-004、API-001。
- [x] 实现 React Router、error boundary、Suspense、Semi ConfigProvider、中文默认/i18n。
- [x] 定义颜色、间距、密度、暗色和可访问 token；不 fork Semi 源码。
- [x] 建立登录布局、管理布局、导航、面包屑、403/404/故障页。
**测试：** 键盘导航、主题、错误隔离、窄屏最小支持和 axe smoke。

### WEB-002：Typed API client 与服务器状态
**前置：** API-001。
- [x] 从锁定 OpenAPI 生成 client，生成物禁止手改。
- [x] TanStack Query 管服务器状态；query key 必含 tenant；Zustand 仅存 UI/session 偏好。
- [x] 统一处理 problem+json、401 refresh single-flight、403、409/412 和 429 Retry-After。
**测试：** tenant 切换清 cache、并发 refresh、取消旧请求、错误映射。

### WEB-003：路由和权限壳
**前置：** AUTH-003、WEB-002。
- [x] 路由/菜单声明 permission 和 capability；服务端返回有效 capability 集。
- [x] `Can` 组件只控制 UX；mutation 前不依赖本地判断代替服务端。
- [x] 会话结束销毁 query cache、SSE、播放器和敏感临时状态。
**测试：** 权限变化、深链接、租户切换、登出资源清理。

