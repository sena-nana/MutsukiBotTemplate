use std::path::PathBuf;
use std::time::Duration;

use example_bot::assemble_service;
use mutsuki_service_config::{ConfigOverrides, ServiceConfig};
use mutsuki_service_control::ControlMethod;
use serde_json::Value;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires a local QQBot config, Host secret and manual /ping plus /echo messages"]
async fn real_qqbot_ping_and_echo_smoke() {
    let Some(path) = std::env::var_os("MUTSUKI_QQBOT_SMOKE_CONFIG").map(PathBuf::from) else {
        eprintln!("SKIPPED: set MUTSUKI_QQBOT_SMOKE_CONFIG and the referenced Host secret");
        return;
    };
    let service = ServiceConfig::load(ConfigOverrides {
        config_file: Some(path),
        ..Default::default()
    })
    .expect("load local QQBot smoke config");
    assert!(
        service.ipc.enabled,
        "real smoke requires IPC health/task inspection"
    );
    let control_config = service.clone();
    let runtime = assemble_service(service)
        .expect("assemble configured QQBot")
        .start()
        .await
        .expect("start real QQBot ServiceRuntime");

    eprintln!("Send /ping and /echo hello to the configured QQ group now.");
    let timeout_secs = std::env::var("MUTSUKI_QQBOT_SMOKE_TIMEOUT_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(120);
    let result = tokio::time::timeout(Duration::from_secs(timeout_secs), async {
        loop {
            let tasks = control(&control_config, ControlMethod::TaskList).await;
            let completed_sends = tasks
                .as_array()
                .into_iter()
                .flatten()
                .filter(|task| {
                    task["protocol_id"] == "mutsuki.bot.message/send@1"
                        && task["status"] == "completed"
                })
                .count();
            if completed_sends >= 2 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    })
    .await;

    runtime.shutdown().await;
    result.expect("real /ping and /echo did not complete before the smoke timeout");
}

async fn control(config: &ServiceConfig, method: ControlMethod) -> Value {
    let client = mutsuki_service_ipc::ControlClient::new(config.into());
    let response = client.request(method, Value::Null).await.unwrap();
    assert!(response.ok, "control failed: {:?}", response.error);
    response.result.unwrap_or(Value::Null)
}
