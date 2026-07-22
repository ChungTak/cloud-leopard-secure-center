use std::env;
use std::process::ExitCode;

fn main() -> ExitCode {
    let _config_path = env::var("CLSC_CONFIG").unwrap_or_else(|_| "config.toml".into());
    let _v_application = application::version();
    let _v_storage_postgres = storage_postgres::version();
    let _v_message_local = message_local::version();
    let _v_foundation = foundation::version();
    println!(r#"{{"status":"healthy","component":"migration-cli"}}"#);
    ExitCode::SUCCESS
}
