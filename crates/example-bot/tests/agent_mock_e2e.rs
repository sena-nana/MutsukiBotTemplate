#![cfg(feature = "agent-bot")]

use std::time::Duration;

use example_bot::plugin;
use mutsuki_agent_bundle::{
    AgentLoop, AgentPluginBundle, AgentRuntimeRunner, HttpModelProvider, HttpModelProviderOptions,
    ModelGateway,
};
use mutsuki_bot_protocol::{
    BOT_COMMAND_PARSE_PROTOCOL_ID, BotAccountRef, BotEvent, BotEventKind, BotEventSubscription,
    BotMessage, BotPlatform, BotTarget,
};
use mutsuki_plugin_bot_command::{BotCommandRunner, bot_command_manifest};
use mutsuki_plugin_bot_event_router::{BotEventRouterRunner, bot_event_router_manifest};
use mutsuki_runtime_contracts::{Task, TaskBatch};
use mutsuki_service_config::{IpcTransport, ServiceConfig};
use mutsuki_service_control::ControlMethod;
use mutsuki_service_runtime::{
    HostEventSource, HostEventSourceContext, HostEventSourceDescriptor, HostEventSourceFuture,
    HostEventSourceHealth, ServiceRuntimeBuilder,
};
use serde_json::{Value, json};
use tokio::net::TcpListener;

#[path = "support/agent_tool.rs"]
mod agent_tool;
#[path = "support/mock_transport.rs"]
mod mock_transport;

fn assemble_mock_agent_service(service: ServiceConfig) -> ServiceRuntimeBuilder {
    assemble_agent_service(service, AgentPluginBundle::default())
}

fn assemble_agent_service(
    service: ServiceConfig,
    agent: AgentPluginBundle,
) -> ServiceRuntimeBuilder {
    agent.tools.register(agent_tool::tool_descriptor()).unwrap();
    install_agent_bundle(
        ServiceRuntimeBuilder::new(service)
            .register_builtin_plugin(bot_event_router_manifest(1))
            .register_builtin_plugin(bot_command_manifest(1))
            .register_builtin_plugin(plugin::manifest(1))
            .register_builtin_plugin(mock_transport::manifest(1))
            .register_builtin_plugin(agent_tool::manifest(1))
            .register_builtin_runner(move || {
                Box::new(BotEventRouterRunner::new(1, vec![command_subscription()]))
            })
            .register_builtin_runner(|| Box::new(BotCommandRunner::new(1, vec!["/".into()])))
            .register_builtin_runner(|| plugin::runner(1))
            .register_builtin_runner(|| mock_transport::runner(1))
            .register_builtin_runner(|| agent_tool::runner(1)),
        agent,
    )
}

fn install_agent_bundle(
    mut builder: ServiceRuntimeBuilder,
    agent: AgentPluginBundle,
) -> ServiceRuntimeBuilder {
    for manifest in agent.manifests() {
        builder = builder.register_builtin_plugin(manifest);
    }
    for kind in AgentRuntimeRunner::ALL {
        let agent = agent.clone();
        builder = builder
            .register_runtime_client_runner(move |client| agent.runtime_runner(kind, client));
    }
    let effect = agent.clone();
    builder = builder.register_fallible_builtin_runner(move || {
        let handle = tokio::runtime::Handle::try_current().map_err(|error| error.to_string())?;
        Ok::<_, String>(effect.http_effect_runner(handle))
    });
    let poll = agent.clone();
    builder.register_builtin_runner(move || poll.model_poll_runner())
}

fn command_subscription() -> BotEventSubscription {
    BotEventSubscription {
        subscription_id: "message-to-command".into(),
        handler_protocol_id: BOT_COMMAND_PARSE_PROTOCOL_ID.into(),
        handler_binding_id: None,
        platform: None,
        event_kind: Some(BotEventKind::MessageCreated),
    }
}

struct AskSource {
    descriptor: HostEventSourceDescriptor,
    commands: Vec<(&'static str, &'static str)>,
}

impl AskSource {
    fn new() -> Self {
        Self {
            descriptor: HostEventSourceDescriptor::new("template-agent-test", "template.test"),
            commands: vec![
                ("ask", "/ask hello from bot"),
                ("tool", "/ask-tool use echo"),
            ],
        }
    }

    fn one(text: &'static str) -> Self {
        Self {
            descriptor: HostEventSourceDescriptor::new("template-agent-test", "template.test"),
            commands: vec![("ask", text)],
        }
    }
}

impl HostEventSource for AskSource {
    fn descriptor(&self) -> &HostEventSourceDescriptor {
        &self.descriptor
    }

    fn start(&mut self, mut ctx: HostEventSourceContext) -> HostEventSourceFuture {
        let commands = self.commands.clone();
        Box::pin(async move {
            ctx.task_submitter.submit_batch(TaskBatch {
                batch_id: "template-agent-test".into(),
                tick_id: None,
                tasks: commands
                    .into_iter()
                    .map(|(id, text)| command_task(id, text))
                    .collect(),
                resource_plan: None,
            })?;
            ctx.shutdown.cancelled().await;
            Ok(())
        })
    }

    fn shutdown(&mut self) -> HostEventSourceFuture {
        Box::pin(async { Ok(()) })
    }

    fn health(&self) -> HostEventSourceHealth {
        HostEventSourceHealth::Healthy
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn http_model_failure_isolated_from_bot_send() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let mut bytes = [0_u8; 4096];
        let _ = stream.read(&mut bytes).await.unwrap();
        stream
            .write_all(
                b"HTTP/1.1 400 Bad Request\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}",
            )
            .await
            .unwrap();
    });
    let (service, _home) = service_config().await;
    let control_config = service.clone();
    let bundle = http_bundle(format!("http://{address}/generate"), 1_000);
    let runtime = assemble_agent_service(service, bundle)
        .register_event_source(Box::new(AskSource::one("/ask fail")))
        .start()
        .await
        .unwrap();

    let tasks = wait_for_terminal_agent(&control_config).await;
    assert!(
        !tasks
            .as_array()
            .unwrap()
            .iter()
            .any(|task| { task["protocol_id"] == "mutsuki.bot.message/send@1" })
    );
    assert!(!tasks.to_string().contains("TEST_MODEL_SECRET"));
    shutdown(runtime, &control_config).await;
    server.await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn http_model_timeout_is_bounded() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (_stream, _) = listener.accept().await.unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;
    });
    let (service, _home) = service_config().await;
    let control_config = service.clone();
    let bundle = http_bundle(format!("http://{address}/generate"), 20);
    let runtime = assemble_agent_service(service, bundle)
        .register_event_source(Box::new(AskSource::one("/ask timeout")))
        .start()
        .await
        .unwrap();

    tokio::time::timeout(
        Duration::from_secs(2),
        wait_for_terminal_agent(&control_config),
    )
    .await
    .expect("model timeout reaches terminal task");
    shutdown(runtime, &control_config).await;
    server.await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cancelling_http_effect_allows_clean_shutdown() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (accepted_tx, accepted_rx) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (_stream, _) = listener.accept().await.unwrap();
        let _ = accepted_tx.send(());
        tokio::time::sleep(Duration::from_secs(5)).await;
    });
    let (service, _home) = service_config().await;
    let control_config = service.clone();
    let bundle = http_bundle(format!("http://{address}/generate"), 10_000);
    let runtime = assemble_agent_service(service, bundle)
        .register_event_source(Box::new(AskSource::one("/ask cancel")))
        .start()
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(2), accepted_rx)
        .await
        .expect("HTTP effect started")
        .unwrap();
    let effect_id = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let tasks = control(&control_config, ControlMethod::TaskList).await;
            if let Some(id) = tasks.as_array().unwrap().iter().find_map(|task| {
                (task["protocol_id"] == "effect.mutsuki.agent.model/http@1")
                    .then(|| task["task_id"].as_str().map(str::to_owned))
                    .flatten()
            }) {
                break id;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("effect task appears");
    control_with(
        &control_config,
        ControlMethod::TaskCancel,
        json!({"id": effect_id}),
    )
    .await;
    tokio::time::timeout(Duration::from_secs(2), shutdown(runtime, &control_config))
        .await
        .expect("cancelled request leaves no shutdown worker");
    server.abort();
    let _ = server.await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn service_runtime_routes_ask_callback_and_no_side_effect_tool() {
    let (service, _home) = service_config().await;
    let control_config = service.clone();
    let runtime = assemble_mock_agent_service(service)
        .register_event_source(Box::new(AskSource::new()))
        .start()
        .await
        .unwrap();

    let tasks = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let tasks = control(&control_config, ControlMethod::TaskList).await;
            let values = tasks.as_array().unwrap();
            let sends = values
                .iter()
                .filter(|task| task["protocol_id"] == "mutsuki.bot.message/send@1")
                .count();
            if sends == 2 {
                break tasks;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("two Agent replies completed");

    let protocols = tasks
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|task| task["protocol_id"].as_str())
        .collect::<Vec<_>>();
    for required in [
        BOT_COMMAND_PARSE_PROTOCOL_ID,
        "mutsuki.bot.command/handle@1",
        "mutsuki.agent/run@1",
        "mutsuki.agent.model/generate@1",
        "template.agent.result/handle@1",
        "mutsuki.agent.tool/execute@1",
        "template.agent.tool/echo@1",
        "mutsuki.bot.message/send@1",
    ] {
        assert!(
            protocols.contains(&required),
            "missing {required}: {protocols:?}"
        );
    }
    assert!(tasks.to_string().contains("trace-tool"));
    assert!(!tasks.to_string().contains("AGENT_MODEL_API_KEY"));

    let health = control(&control_config, ControlMethod::HealthCheck).await;
    assert_eq!(health["service"], "ok");
    assert_eq!(health["core"], "ok");
    assert_eq!(health["components"], json!({}));
    let reload = control(&control_config, ControlMethod::PluginReload).await;
    assert_eq!(reload["previous_generation"], 1);
    assert_eq!(reload["registry_generation"], 2);
    assert_eq!(reload["runner_errors"], json!([]));
    let reloaded_health = control(&control_config, ControlMethod::HealthCheck).await;
    assert_eq!(reloaded_health["service"], "ok");
    assert_eq!(reloaded_health["core"], "ok");

    shutdown(runtime, &control_config).await;
}

fn http_bundle(endpoint: String, timeout_ms: u64) -> AgentPluginBundle {
    let options = HttpModelProviderOptions {
        provider_id: "http".into(),
        endpoint,
        default_model: "test-model".into(),
        timeout_ms,
        max_retries: 0,
    };
    let gateway = ModelGateway::with_default_provider("http");
    gateway.register(std::sync::Arc::new(
        HttpModelProvider::new(options, "TEST_MODEL_SECRET").unwrap(),
    ));
    AgentPluginBundle {
        agent_loop: AgentLoop::default().with_default_model("test-model"),
        model: gateway,
        ..AgentPluginBundle::default()
    }
}

async fn wait_for_terminal_agent(config: &ServiceConfig) -> Value {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let tasks = control(config, ControlMethod::TaskList).await;
            if tasks.as_array().unwrap().iter().any(|task| {
                task["protocol_id"] == "mutsuki.agent/run@1"
                    && matches!(task["status"].as_str(), Some("failed" | "cancelled"))
            }) {
                break tasks;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("Agent task reaches failure or cancellation")
}

async fn shutdown(runtime: mutsuki_service_runtime::ServiceRuntime, config: &ServiceConfig) {
    control(config, ControlMethod::ServiceShutdown).await;
    runtime
        .run_until_shutdown_signal(std::future::pending::<String>())
        .await
        .unwrap();
}

fn command_task(id: &str, text: &str) -> Task {
    let target = BotTarget::User {
        user_id: "local-user".into(),
    };
    let event = BotEvent {
        event_id: format!("event-{id}"),
        platform: BotPlatform::Custom("mock".into()),
        bot: BotAccountRef {
            account_id: "local-bot".into(),
            platform: BotPlatform::Custom("mock".into()),
        },
        kind: BotEventKind::MessageCreated,
        time_ms: 0,
        target: target.clone(),
        actor: None,
        message: Some(BotMessage::text(target, text)),
        raw: None,
        ext: Default::default(),
    };
    let mut task = Task::new(
        format!("source-{id}"),
        BOT_COMMAND_PARSE_PROTOCOL_ID,
        serde_json::to_value(event).unwrap(),
    );
    task.trace_id = Some(format!("trace-{id}"));
    task.correlation_id = Some(format!("correlation-{id}"));
    task
}

async fn service_config() -> (ServiceConfig, tempfile::TempDir) {
    let home = tempfile::tempdir().unwrap();
    let probe = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = probe.local_addr().unwrap();
    drop(probe);
    let mut service = ServiceConfig::default();
    service.service.instance_id = "template-agent-test".into();
    service.service.home_dir = home.path().to_path_buf();
    service.service.log_dir = home.path().join("logs");
    service.service.run_dir = home.path().join("run");
    service.plugins.dynamic_dirs.clear();
    service.plugins.disabled_dir = home.path().join("disabled");
    service.observe.console = false;
    service.ipc.enabled = true;
    service.ipc.transport = IpcTransport::TcpDebug;
    service.ipc.tcp_debug_addr = Some(address.to_string());
    service.ipc.token = Some("test-token".into());
    std::fs::create_dir_all(&service.service.log_dir).unwrap();
    std::fs::create_dir_all(&service.service.run_dir).unwrap();
    (service, home)
}

async fn control(config: &ServiceConfig, method: ControlMethod) -> Value {
    control_with(config, method, Value::Null).await
}

async fn control_with(config: &ServiceConfig, method: ControlMethod, params: Value) -> Value {
    let client = mutsuki_service_ipc::ControlClient::new(config.into());
    let response = client.request(method, params).await.unwrap();
    assert!(response.ok, "control failed: {:?}", response.error);
    response.result.unwrap_or(Value::Null)
}
