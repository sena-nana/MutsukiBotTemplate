use std::path::Path;

use example_bot::assemble_service;
use mutsuki_bot_service_host_integration::QqBotPluginBundle;
use mutsuki_plugin_bot_adapter_qqbot::{MediaChunk, QqBotConfig, QqMediaError, QqMediaProvider};
use mutsuki_service_config::{ConfigOverrides, ServiceConfig};
use tempfile::tempdir;

struct NoopMediaProvider;

impl QqMediaProvider for NoopMediaProvider {
    fn read_chunks(
        &mut self,
        _resource_ref: &str,
        _block_size: u64,
    ) -> Result<Vec<MediaChunk>, QqMediaError> {
        Ok(Vec::new())
    }
}

#[tokio::test]
async fn external_service_config_starts_only_the_neutral_business_plugin() {
    let root = tempdir().unwrap();
    let config_path = root.path().join("product.toml");
    std::fs::write(&config_path, service_toml(root.path())).unwrap();
    let service = ServiceConfig::load(ConfigOverrides {
        config_file: Some(config_path),
        ..Default::default()
    })
    .unwrap();

    let runtime = assemble_service(service).start().await.unwrap();
    runtime.shutdown().await;
}

#[tokio::test]
async fn configured_qq_integration_fails_preflight_without_host_secret() {
    let root = tempdir().unwrap();
    let config_path = root.path().join("product.toml");
    std::fs::write(&config_path, service_toml(root.path())).unwrap();
    let service = ServiceConfig::load(ConfigOverrides {
        config_file: Some(config_path),
        ..Default::default()
    })
    .unwrap();
    let mut qq = QqBotConfig::new("configured-account", "configured-app");
    qq.client_secret_key = "MISSING_TEMPLATE_QQ_SECRET".into();
    let bundle = QqBotPluginBundle::new(qq, || Box::new(NoopMediaProvider)).unwrap();
    let builder = bundle.install(assemble_service(service)).unwrap();

    let error = match builder.start().await {
        Ok(runtime) => {
            runtime.shutdown().await;
            panic!("QQ integration started without required Host secret")
        }
        Err(error) => error,
    };
    assert!(error.to_string().contains("MISSING_TEMPLATE_QQ_SECRET"));
}

fn service_toml(root: &Path) -> String {
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

[observe]
console = false
json = false
log_file = "service.log"
panic_file = "panic.log"
"#,
        root.to_string_lossy().replace('\\', "/")
    )
}
