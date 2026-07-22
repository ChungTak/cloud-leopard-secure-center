# security-platform

Binary assembly entry point: config loading, lifecycle, health service and listener bootstrap.

允许依赖：application, http-api, observability, foundation。
禁止：SQL/Axum internals, business aggregates, global mutable state。
