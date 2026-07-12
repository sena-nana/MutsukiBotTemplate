use std::path::PathBuf;

use example_bot::assemble_service;
use mutsuki_service_config::{ConfigOverrides, ServiceConfig};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config_path = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("usage: example-bot <service-config.toml>")?;
    let service = ServiceConfig::load(ConfigOverrides {
        config_file: Some(config_path),
        ..Default::default()
    })?;
    assemble_service(service)?
        .start()
        .await?
        .run_foreground()
        .await?;
    Ok(())
}
