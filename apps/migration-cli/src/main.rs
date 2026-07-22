use foundation::PlatformError;
use sqlx::postgres::PgPoolOptions;
use std::env;
use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/clsc".to_string());

    match run_migrations(&database_url).await {
        Ok(()) => {
            println!(r#"{{"status":"ready","component":"migration-cli"}}"#);
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!(r#"{{"status":"error","message":"{}"}}"#, e);
            ExitCode::FAILURE
        }
    }
}

async fn run_migrations(database_url: &str) -> Result<(), PlatformError> {
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(database_url)
        .await
        .map_err(|e| PlatformError::invalid("database_url", e.to_string()))?;

    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .map_err(|e| PlatformError::invalid("migration", e.to_string()))?;

    pool.close().await;
    Ok(())
}
