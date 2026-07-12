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

    let gateway_health = tokio::time::timeout(Duration::from_secs(45), async {
        loop {
            let health = control(&control_config, ControlMethod::HealthCheck).await;
            if health["event_sources"] == "ok" {
                break health;
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    })
    .await;
    let health = match gateway_health {
        Ok(health) => health,
        Err(_) => {
            let health = control(&control_config, ControlMethod::HealthCheck).await;
            runtime.shutdown().await;
            panic!("QQ Gateway did not become healthy: {health}");
        }
    };
    eprintln!("QQ Gateway healthy: {}", health["event_sources"]);

    eprintln!("Send /ping and /echo hello to the configured QQ chat now.");
    let timeout_secs = std::env::var("MUTSUKI_QQBOT_SMOKE_TIMEOUT_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(120);
    let send_tasks = tokio::time::timeout(Duration::from_secs(timeout_secs), async {
        loop {
            let tasks = control(&control_config, ControlMethod::TaskList).await;
            let sends = tasks
                .as_array()
                .into_iter()
                .flatten()
                .filter(|task| task["protocol_id"] == "mutsuki.bot.message/send@1")
                .cloned()
                .collect::<Vec<_>>();
            if sends.len() >= 2
                && sends
                    .iter()
                    .all(|task| matches!(task["status"].as_str(), Some("completed" | "failed")))
            {
                break sends;
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    })
    .await;

    let diagnostics = match send_tasks {
        Ok(tasks) if tasks.iter().all(|task| task["status"] == "completed") => None,
        Ok(tasks) => {
            let mut outcomes = Vec::new();
            for task in &tasks {
                outcomes.push(
                    control_with_params(
                        &control_config,
                        ControlMethod::TaskOutcome,
                        serde_json::json!({"id": task["task_id"]}),
                    )
                    .await,
                );
            }
            Some((
                control(&control_config, ControlMethod::HealthCheck).await,
                Value::Array(tasks),
                Value::Array(outcomes),
            ))
        }
        Err(_) => Some((
            control(&control_config, ControlMethod::HealthCheck).await,
            control(&control_config, ControlMethod::TaskList).await,
            Value::Null,
        )),
    };
    runtime.shutdown().await;
    if let Some((health, tasks, outcomes)) = diagnostics {
        panic!(
            "real /ping and /echo failed or timed out; health={health}; tasks={tasks}; outcomes={outcomes}"
        );
    }
}

async fn control(config: &ServiceConfig, method: ControlMethod) -> Value {
    control_with_params(config, method, Value::Null).await
}

async fn control_with_params(
    config: &ServiceConfig,
    method: ControlMethod,
    params: Value,
) -> Value {
    let client = mutsuki_service_ipc::ControlClient::new(config.into());
    let response = client.request(method, params).await.unwrap();
    assert!(response.ok, "control failed: {:?}", response.error);
    response.result.unwrap_or(Value::Null)
}
