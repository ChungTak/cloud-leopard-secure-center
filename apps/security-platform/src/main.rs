use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use axum::{Extension, Router};
use foundation::config::RateLimitConfig;
use http_api::client_ip::TrustedProxyConfig;
use http_api::rate_limit::RateLimitState;
use tokio::net::TcpListener;
use tower_http::services::ServeDir;

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
        .and_then(|v| v.parse().ok())
        .unwrap_or(8080);
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
    let proxy_config = TrustedProxyConfig::default();

    let api = http_api::middleware::with_middleware(http_api::routes::router())
        .layer(Extension(rate_limit_state))
        .layer(Extension(proxy_config));
    let app = if static_dir.is_dir() {
        let serve = ServeDir::new(&static_dir).append_index_html_on_directories(true);
        Router::new().nest("/api/v1", api).fallback_service(serve)
    } else {
        api
    };

    let addr: SocketAddr = ([0, 0, 0, 0], port).into();
    let listener = TcpListener::bind(addr).await?;
    println!("security-platform listening on {addr}");
    axum::serve(listener, app).await?;
    Ok(())
}
