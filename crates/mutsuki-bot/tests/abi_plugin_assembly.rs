use std::fs;
use std::process::Command;

use mutsuki_bot::assemble_service;
use mutsuki_service_config::{ConfigOverrides, ServiceConfig};
use mutsuki_service_plugin_loader::PluginToml;
use sha2::{Digest, Sha256};
use tempfile::tempdir;

#[tokio::test]
async fn external_core_abi_artifact_starts_in_real_service_runtime() {
    let root = tempdir().unwrap();
    let metadata = Command::new(env!("CARGO"))
        .args(["metadata", "--locked", "--format-version", "1"])
        .output()
        .expect("read Cargo metadata");
    assert!(metadata.status.success());
    let metadata: serde_json::Value = serde_json::from_slice(&metadata.stdout).unwrap();
    let fixture_manifest = metadata["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|package| package["name"] == "mutsuki-service-abi-fixture")
        .and_then(|package| package["manifest_path"].as_str())
        .expect("fixture manifest in dependency metadata");
    let fixture_target = root.path().join("fixture-target");
    let build = Command::new(env!("CARGO"))
        .args(["build", "--manifest-path", fixture_manifest, "-p"])
        .arg("mutsuki-service-abi-fixture")
        .arg("--target-dir")
        .arg(&fixture_target)
        .status()
        .expect("build Core ABI fixture");
    assert!(build.success());
    let file_name = if cfg!(target_os = "windows") {
        "mutsuki_service_abi_fixture.dll"
    } else if cfg!(target_os = "macos") {
        "libmutsuki_service_abi_fixture.dylib"
    } else {
        "libmutsuki_service_abi_fixture.so"
    };
    let artifact = fixture_target.join("debug").join(file_name);
    assert!(artifact.is_file(), "ABI fixture: {}", artifact.display());

    let installed = root.path().join("plugins").join("installed");
    let plugin_dir = installed.join("abi-fixture");
    fs::create_dir_all(&plugin_dir).unwrap();
    let installed_artifact = plugin_dir.join(file_name);
    fs::copy(&artifact, &installed_artifact).unwrap();
    let sha256 = format!(
        "sha256:{:x}",
        Sha256::digest(fs::read(&installed_artifact).unwrap())
    );
    let manifest = mutsuki_service_abi_fixture::fixture_manifest(file_name, &sha256);
    fs::write(
        plugin_dir.join("plugin.toml"),
        toml::to_string(&PluginToml {
            manifest,
            runtime: None,
            enabled: Some(true),
        })
        .unwrap(),
    )
    .unwrap();

    let config_path = root.path().join("product.toml");
    fs::write(
        &config_path,
        format!(
            r#"[service]
profile = "test"
instance_id = "template-abi-test"
home_dir = "{}"
data_dir = "data"
log_dir = "logs"
plugin_dir = "plugins"
run_dir = "run"

[ipc]
enabled = false
transport = "named-pipe"
name = "template-abi-test"
token = "test-token"

[plugins]
builtin = []
dynamic_dirs = ["{}"]
disabled_dir = "plugins/disabled"

[observe]
console = false
json = false
log_file = "service.log"
panic_file = "panic.log"
"#,
            root.path().to_string_lossy().replace('\\', "/"),
            installed.to_string_lossy().replace('\\', "/"),
        ),
    )
    .unwrap();
    let service = ServiceConfig::load(ConfigOverrides {
        config_file: Some(config_path),
        ..Default::default()
    })
    .unwrap();

    let runtime = assemble_service(service).unwrap().start().await.unwrap();
    runtime.shutdown().await;
}
