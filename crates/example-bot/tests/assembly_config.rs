use std::path::Path;

use example_bot::assemble_service;
use mutsuki_service_config::{ConfigOverrides, ServiceConfig};
use tempfile::tempdir;

#[tokio::test]
async fn external_service_config_starts_only_the_neutral_business_plugin() {
    let root = tempdir().unwrap();
    let config_path = root.path().join("product.toml");
    std::fs::write(&config_path, service_toml(root.path(), "")).unwrap();
    let service = load(&config_path);

    let runtime = assemble_service(service).unwrap().start().await.unwrap();
    runtime.shutdown().await;
}

#[tokio::test]
async fn configured_qq_integration_fails_preflight_without_host_secret() {
    let root = tempdir().unwrap();
    let config_path = root.path().join("product.toml");
    std::fs::write(
        &config_path,
        service_toml(
            root.path(),
            r#"
[[plugins.configured]]
id = "mutsuki.bot.adapter.qqbot"

[plugins.configured.config]
account_id = "configured-account"
app_id = "configured-app"
client_secret_key = "MISSING_TEMPLATE_QQ_SECRET"
"#,
        ),
    )
    .unwrap();
    let service = load(&config_path);

    let error = match assemble_service(service).unwrap().start().await {
        Ok(runtime) => {
            runtime.shutdown().await;
            panic!("QQ integration started without required Host secret")
        }
        Err(error) => error,
    };
    assert!(error.to_string().contains("MISSING_TEMPLATE_QQ_SECRET"));
}

fn load(path: &Path) -> ServiceConfig {
    ServiceConfig::load(ConfigOverrides {
        config_file: Some(path.to_path_buf()),
        ..Default::default()
    })
    .unwrap()
}

fn service_toml(root: &Path, configured: &str) -> String {
    format!(
        r#"[service]
profile = "test"
instance_id = "template-test"
home_dir = "{}"
data_dir = "data"
log_dir = "logs"
plugin_dir = "plugins"
run_dir = "run"

[ipc]
enabled = false
transport = "named-pipe"
name = "template-test"
token = "test-token"

[plugins]
builtin = []
dynamic_dirs = []
disabled_dir = "disabled"
{}

[observe]
console = false
json = false
log_file = "service.log"
panic_file = "panic.log"
"#,
        root.to_string_lossy().replace('\\', "/"),
        configured
    )
}
