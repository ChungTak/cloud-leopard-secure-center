use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use application::auth::Authenticator;
use axum::{Extension, Router};
use foundation::{
    SystemClock, SystemRandom,
    config::{Port, RateLimitConfig},
};
use http_api::auth::DenyAllAuthenticator;
use http_api::client_ip::TrustedProxyConfig;
use http_api::idempotency::IdempotencyState;
use http_api::pagination::PaginationConfig;
use http_api::rate_limit::RateLimitState;
use tokio::net::TcpListener;
use tower_http::services::{ServeDir, ServeFile};

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    if args.get(1).is_some_and(|a| a == "health") {
        println!(r#"{{"status":"healthy","component":"security-platform"}}"#);
        return ExitCode::SUCCESS;
    }

    let runtime = match tokio::runtime::Runtime::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("failed to create tokio runtime: {e}");
            return ExitCode::FAILURE;
        }
    };

    if let Err(e) = runtime.block_on(start_server()) {
        eprintln!("server error: {e}");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}

async fn start_server() -> Result<(), Box<dyn std::error::Error>> {
    let port: u16 = env::var("CLSC_HTTP_PORT")
        .ok()
        .map(|v| v.parse::<u16>())
        .transpose()
        .map_err(|_| "CLSC_HTTP_PORT must be a valid u16")?
        .map(Port::new)
        .transpose()
        .map_err(|_| "CLSC_HTTP_PORT must be at least 1024")?
        .unwrap_or(Port::new(8080).map_err(|_| "default port invalid")?)
        .value();
    let static_dir: PathBuf = env::var("CLSC_STATIC_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/var/www/static"));

    let rate_limit_state = Arc::new(RateLimitState::new(
        RateLimitConfig {
            requests: 60,
            window_seconds: 60,
        },
        RateLimitConfig {
            requests: 1000,
            window_seconds: 60,
        },
    ));

    let proxy_config = env::var("CLSC_TRUSTED_PROXIES")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .map(|v| {
            let raw: Vec<String> = v.split(',').map(|s| s.trim().to_string()).collect();
            TrustedProxyConfig::parse(&raw)
        })
        .transpose()?
        .unwrap_or_default();

    let cursor_secret = env::var("CLSC_CURSOR_SECRET")
        .ok()
        .filter(|s| !s.is_empty())
        .map(|s| s.into_bytes())
        .ok_or("CLSC_CURSOR_SECRET must be set to a non-empty value")?;
    if cursor_secret.len() < 32 {
        return Err("CLSC_CURSOR_SECRET must be at least 32 bytes".into());
    }
    let pagination_config = Arc::new(PaginationConfig::new(100, cursor_secret)?);

    let cors_allowed_origins = env::var("CLSC_CORS_ALLOWED_ORIGINS")
        .ok()
        .filter(|s| !s.is_empty())
        .map(|v| {
            v.split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from)
                .collect::<Vec<_>>()
        })
        .or_else(|| Some(vec![]));

    let idempotency_state = Arc::new(IdempotencyState::new(Duration::from_secs(24 * 60 * 60)));

    let api = http_api::middleware::with_middleware(
        http_api::routes::router(),
        cors_allowed_origins,
        SystemClock,
        SystemRandom,
    )
    .layer(Extension(rate_limit_state))
    .layer(Extension(proxy_config))
    .layer(Extension(pagination_config))
    .layer(Extension(idempotency_state))
    .layer(Extension::<Arc<dyn Authenticator>>(Arc::new(
        DenyAllAuthenticator,
    )));
    let app = if static_dir.is_dir() {
        let index = static_dir.join("index.html");
        let serve = ServeDir::new(&static_dir)
            .append_index_html_on_directories(true)
            .fallback(ServeFile::new(index));
        Router::new().nest("/api/v1", api).fallback_service(serve)
    } else {
        api
    };

    let addr: SocketAddr = ([0, 0, 0, 0], port).into();
    let listener = TcpListener::bind(addr).await?;
    println!("security-platform listening on {addr}");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;
    Ok(())
}
