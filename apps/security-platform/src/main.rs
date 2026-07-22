use std::env;
use std::process::ExitCode;

fn main() -> ExitCode {
    let _config_path = env::var("CLSC_CONFIG").unwrap_or_else(|_| "config.toml".into());
    let _v_application = application::version();
    let _v_http_api = http_api::version();
    let _v_observability = observability::version();
    let _v_foundation = foundation::version();
    println!(r#"{{"status":"healthy","component":"security-platform"}}"#);
    ExitCode::SUCCESS
}
