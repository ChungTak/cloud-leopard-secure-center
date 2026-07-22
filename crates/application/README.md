# application

Application layer: use cases, transactions, permission checks, projection, outbox.

允许依赖：domain-identity, domain-organization, domain-authorization, domain-resource, domain-audit, domain-configuration, storage-api, message-api, foundation。
禁止：Tokio/Axum/Tonic/SQLx/NATS/HTTP DTO/global state。
