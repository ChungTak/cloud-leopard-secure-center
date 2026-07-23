# 15. 第一阶段验收与打包

### PKG-001：单机 Compose 与安装检查
**前置：** API-003、WEB-007、APP-003。
- [x] 构建平台镜像和静态 Web 资源；非 root、只读 rootfs、healthcheck。
- [x] Compose 包含 platform/PostgreSQL/OTel，可使用外部数据库；secret 不写默认文件。
- [x] 安装前检查端口、磁盘、时钟、数据库版本、备份目录和证书。
**测试：** 全新离线安装、重启、错误配置、数据库暂不可用、升级前备份。

### TST-001：Phase 1 端到端验收
**前置：** PKG-001。
- [ ] 自动化正式方案 18.2 的十个场景，包括 closure、scope、revision、幂等、RLS、审计和 tenant cache。
- [ ] 使用真实 PostgreSQL 和浏览器；测试有 deadline、清理和独立 tenant fixture。
- [ ] 生成可复现报告，记录 commit、镜像 digest、硬件和配置。

### PKG-002：首期发布门禁
**前置：** TST-001。
- [ ] 运行全 workspace、OpenAPI breaking、migration、SBOM、容器和许可证扫描。
- [ ] 验证备份恢复后审计、用户和资源数量/digest。
- [ ] 发布 `0.1.0`，包含 checksum、SBOM、配置参考、升级/回滚说明。

## 完成条件
单个业务二进制与 PostgreSQL 完成管理闭环；signaling/media/plugin 未实现能力明确返回 `UNSUPPORTED`，无占位成功。
