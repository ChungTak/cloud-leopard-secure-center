//! Trusted proxy and client IP resolution.

use axum::{
    extract::FromRequestParts,
    http::{Extensions, HeaderMap, StatusCode, request::Parts},
};
use foundation::PlatformError;
use ipnet::IpNet;
use std::net::{IpAddr, SocketAddr};

use crate::error::AppError;

/// Parsed trusted proxy configuration.
#[derive(Debug, Clone, Default)]
pub struct TrustedProxyConfig {
    networks: Vec<IpNet>,
}

impl TrustedProxyConfig {
    /// Create a config from raw CIDR or single-IP strings. Empty entries are
    /// ignored; any other invalid entry causes an error so misconfiguration is
    /// not silently accepted.
    pub fn parse(raw: &[String]) -> Result<Self, PlatformError> {
        let mut networks = Vec::with_capacity(raw.len());
        for s in raw {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Ok(net) = trimmed.parse::<IpNet>() {
                networks.push(net);
                continue;
            }
            if let Ok(addr) = trimmed.parse::<IpAddr>() {
                let prefix = if addr.is_ipv4() { 32 } else { 128 };
                if let Ok(net) = IpNet::new(addr, prefix) {
                    networks.push(net);
                    continue;
                }
            }
            return Err(PlatformError::invalid(
                "trusted_proxies",
                format!("{trimmed:?} is not a valid CIDR or IP address"),
            ));
        }
        Ok(Self { networks })
    }

    /// Whether any trusted proxy network contains the given address.
    pub fn is_trusted(&self, addr: IpAddr) -> bool {
        self.networks.iter().any(|net| net.contains(&addr))
    }

    /// Returns true when at least one proxy network is configured.
    pub fn has_proxies(&self) -> bool {
        !self.networks.is_empty()
    }
}

/// Resolved client IP address.
#[derive(Debug, Clone)]
pub struct ClientIp(pub Option<IpAddr>);

impl<S> FromRequestParts<S> for ClientIp
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let config = parts
            .extensions
            .get::<TrustedProxyConfig>()
            .cloned()
            .unwrap_or_default();
        Ok(Self(resolve_client_ip(
            &parts.headers,
            &parts.extensions,
            &config,
        )))
    }
}

/// Resolve the original client IP from `ConnectInfo` and `X-Forwarded-For`.
pub fn resolve_client_ip(
    headers: &HeaderMap,
    extensions: &Extensions,
    config: &TrustedProxyConfig,
) -> Option<IpAddr> {
    let direct = extensions
        .get::<axum::extract::ConnectInfo<SocketAddr>>()
        .map(|info| info.0.ip());

    let forwarded = headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok());

    // Without a direct peer we cannot validate any proxy chain.
    let peer = direct?;

    if !config.has_proxies() {
        return Some(peer);
    }

    if !config.is_trusted(peer) {
        // Peer is not a trusted proxy; ignore X-Forwarded-For to prevent spoofing.
        return Some(peer);
    }

    let forwarded_ips: Vec<IpAddr> = forwarded.map(parse_forwarded_ips).unwrap_or_default();

    // Walk from right (closest to the server) to left (original client),
    // skipping only the consecutive trusted proxies at the end. This
    // prevents an untrusted IP that appears between trusted proxies from
    // being chosen as the client address.
    for ip in forwarded_ips.iter().copied().rev() {
        if !config.is_trusted(ip) {
            return Some(ip);
        }
    }

    // Every proxy in the chain is trusted; the leftmost value is the
    // original client. If the header is empty, the immediate peer wins.
    forwarded_ips.first().copied().or(Some(peer))
}

fn parse_forwarded_ips(text: &str) -> Vec<IpAddr> {
    text.split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect()
}

/// Build an HTTP 400 response for requests with a malformed forwarded-for header.
pub fn bad_client_ip() -> (StatusCode, &'static str) {
    (StatusCode::BAD_REQUEST, "invalid client address")
}
