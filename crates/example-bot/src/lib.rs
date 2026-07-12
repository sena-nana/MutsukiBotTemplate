#[cfg(feature = "agent-bot")]
pub mod agent_tool;
pub mod commands;
pub mod mock_transport;
pub mod plugin;

use std::collections::BTreeMap;
use std::path::Path;
#[cfg(feature = "agent-bot")]
use std::sync::Arc;

use mutsuki_bot_protocol::{BOT_COMMAND_PARSE_PROTOCOL_ID, BotEventKind, BotEventSubscription};
use mutsuki_plugin_bot_adapter_qqbot::{QqBotConfig, QqBotPluginBundle};
use mutsuki_plugin_bot_command::{BotCommandRunner, bot_command_manifest};
use mutsuki_plugin_bot_event_router::{BotEventRouterRunner, bot_event_router_manifest};
use mutsuki_runtime_contracts::{RuntimeProfile, RuntimeProfileMode};
use mutsuki_runtime_host::RuntimeBootstrapper;
use mutsuki_service_config::ServiceConfig;
use mutsuki_service_runtime::ServiceRuntimeBuilder;
use serde::Deserialize;

#[cfg(feature = "agent-bot")]
use mutsuki_agent_bundle::{
    AgentLoop, AgentPluginBundle, AgentRuntimeRunner, HttpModelProvider, HttpModelProviderOptions,
    ModelGateway,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QqBotProfile {
    pub account_id: String,
    pub app_id: String,
    #[serde(default = "default_secret_key")]
    pub client_secret_key: String,
    #[serde(default)]
    pub intents: Option<u64>,
    #[serde(default)]
    pub transport: QqTransportProfile,
    #[serde(default)]
    pub gateway: QqGatewayProfile,
    #[serde(default)]
    pub agent: AgentProfile,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AgentProfile {
    pub provider_id: String,
    pub endpoint: String,
    pub model: String,
    pub secret_key: String,
    pub timeout_ms: u64,
    pub max_retries: u8,
}

impl Default for AgentProfile {
    fn default() -> Self {
        Self {
            provider_id: "http".into(),
            endpoint: "https://api.openai.com/v1/chat/completions".into(),
            model: "gpt-4.1-mini".into(),
            secret_key: "AGENT_MODEL_API_KEY".into(),
            timeout_ms: 30_000,
            max_retries: 1,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QqTransportProfile {
    pub token_url: Option<String>,
    pub openapi_base_url: Option<String>,
    pub request_timeout_ms: Option<u64>,
    pub connect_timeout_ms: Option<u64>,
    pub response_body_limit_bytes: Option<usize>,
    pub token_refresh_margin_secs: Option<u64>,
    pub max_retry_attempts: Option<u8>,
    pub retry_base_delay_ms: Option<u64>,
    pub retry_max_delay_ms: Option<u64>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QqGatewayProfile {
    pub shard: Option<[u64; 2]>,
    pub hello_timeout_ms: Option<u64>,
    pub ack_timeout_ms: Option<u64>,
    pub queue_capacity: Option<usize>,
    pub dedup_window: Option<usize>,
    pub reconnect_initial_delay_ms: Option<u64>,
    pub reconnect_max_delay_ms: Option<u64>,
    pub reconnect_jitter_ms: Option<u64>,
    pub rate_limit_delay_ms: Option<u64>,
}

impl QqBotProfile {
    pub fn load(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let profile: Self = toml::from_str(&std::fs::read_to_string(path)?)?;
        if profile.account_id.trim().is_empty() || profile.app_id.trim().is_empty() {
            return Err("account_id and app_id are required".into());
        }
        Ok(profile)
    }

    pub fn adapter_config(&self) -> QqBotConfig {
        let mut config = QqBotConfig::new(&self.account_id, &self.app_id);
        config.client_secret_key = self.client_secret_key.clone();
        if let Some(intents) = self.intents {
            config.gateway_intents = intents;
        }
        if let Some(value) = &self.transport.token_url {
            config.token_url = value.clone();
        }
        if let Some(value) = &self.transport.openapi_base_url {
            config.openapi_base_url = value.clone();
        }
        set_if_some(
            &mut config.request_timeout_ms,
            self.transport.request_timeout_ms,
        );
        set_if_some(
            &mut config.connect_timeout_ms,
            self.transport.connect_timeout_ms,
        );
        set_if_some(
            &mut config.response_body_limit_bytes,
            self.transport.response_body_limit_bytes,
        );
        set_if_some(
            &mut config.token_refresh_margin_secs,
            self.transport.token_refresh_margin_secs,
        );
        set_if_some(
            &mut config.max_retry_attempts,
            self.transport.max_retry_attempts,
        );
        set_if_some(
            &mut config.retry_base_delay_ms,
            self.transport.retry_base_delay_ms,
        );
        set_if_some(
            &mut config.retry_max_delay_ms,
            self.transport.retry_max_delay_ms,
        );
        set_if_some(&mut config.shard, self.gateway.shard);
        set_if_some(
            &mut config.gateway_hello_timeout_ms,
            self.gateway.hello_timeout_ms,
        );
        set_if_some(
            &mut config.gateway_ack_timeout_ms,
            self.gateway.ack_timeout_ms,
        );
        set_if_some(
            &mut config.gateway_queue_capacity,
            self.gateway.queue_capacity,
        );
        set_if_some(&mut config.gateway_dedup_window, self.gateway.dedup_window);
        set_if_some(
            &mut config.reconnect_initial_delay_ms,
            self.gateway.reconnect_initial_delay_ms,
        );
        set_if_some(
            &mut config.reconnect_max_delay_ms,
            self.gateway.reconnect_max_delay_ms,
        );
        set_if_some(
            &mut config.reconnect_jitter_ms,
            self.gateway.reconnect_jitter_ms,
        );
        set_if_some(
            &mut config.gateway_rate_limit_delay_ms,
            self.gateway.rate_limit_delay_ms,
        );
        config
    }
}

pub fn assemble_real_service(
    service: ServiceConfig,
    profile: &QqBotProfile,
) -> Result<ServiceRuntimeBuilder, Box<dyn std::error::Error>> {
    let subscription = command_subscription();
    let builder = ServiceRuntimeBuilder::new(service.clone())
        .register_builtin_plugin(bot_event_router_manifest(1))
        .register_builtin_plugin(bot_command_manifest(1))
        .register_builtin_plugin(plugin::manifest(1))
        .register_builtin_runner(move || {
            Box::new(BotEventRouterRunner::new(1, vec![subscription.clone()]))
        })
        .register_builtin_runner(|| Box::new(BotCommandRunner::new(1, vec!["/".into()])))
        .register_builtin_runner(|| plugin::runner(1));
    #[cfg(feature = "agent-bot")]
    let builder = {
        let secret = service
            .secret(&profile.agent.secret_key)
            .ok_or_else(|| format!("missing Host secret {}", profile.agent.secret_key))?;
        let agent = http_agent_bundle(
            HttpModelProviderOptions {
                provider_id: profile.agent.provider_id.clone(),
                endpoint: profile.agent.endpoint.clone(),
                default_model: profile.agent.model.clone(),
                timeout_ms: profile.agent.timeout_ms,
                max_retries: profile.agent.max_retries,
            },
            secret,
        )?;
        agent.tools.register(agent_tool::tool_descriptor())?;
        let builder = builder
            .register_builtin_plugin(agent_tool::manifest(1))
            .register_builtin_runner(|| agent_tool::runner(1));
        install_agent_bundle(builder, agent)
    };
    Ok(QqBotPluginBundle::new(profile.adapter_config())?.install(builder)?)
}

#[cfg(feature = "agent-bot")]
pub fn assemble_mock_agent_service(service: ServiceConfig) -> ServiceRuntimeBuilder {
    assemble_agent_service(service, AgentPluginBundle::default())
}

#[cfg(feature = "agent-bot")]
pub fn assemble_agent_service(
    service: ServiceConfig,
    agent: AgentPluginBundle,
) -> ServiceRuntimeBuilder {
    agent
        .tools
        .register(agent_tool::tool_descriptor())
        .expect("template echo tool descriptor is valid");
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

#[cfg(feature = "agent-bot")]
fn http_agent_bundle(
    options: HttpModelProviderOptions,
    secret: String,
) -> mutsuki_agent_protocol::AgentResult<AgentPluginBundle> {
    let provider_id = options.provider_id.clone();
    let default_model = options.default_model.clone();
    let gateway = ModelGateway::with_default_provider(provider_id);
    gateway.register(Arc::new(HttpModelProvider::new(options, secret)?));
    Ok(AgentPluginBundle {
        agent_loop: AgentLoop::default().with_default_model(default_model),
        model: gateway,
        ..AgentPluginBundle::default()
    })
}

#[cfg(feature = "agent-bot")]
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
        let handle = tokio::runtime::Handle::try_current()
            .map_err(|error| format!("Host Tokio runtime is required: {error}"))?;
        Ok::<_, String>(effect.http_effect_runner(handle))
    });
    let poll = agent.clone();
    builder = builder.register_builtin_runner(move || poll.model_poll_runner());
    let health_agent = agent;
    builder.register_health_probe("mutsuki.agent", move || {
        let health = health_agent.model.health_snapshot();
        serde_json::json!({
            "status": if health.ready { "ok" } else { "degraded" },
            "model": {
                "default_provider": health.default_provider,
                "providers": health.providers,
                "ready": health.ready,
            },
            "runners": AgentPluginBundle::runner_ids()
        })
    })
}

pub fn mock_runtime() -> mutsuki_runtime_core::RuntimeResult<mutsuki_runtime_core::CoreRuntime> {
    let mut bootstrapper = RuntimeBootstrapper::new();
    bootstrapper.register_manifest(bot_event_router_manifest(1));
    bootstrapper.register_builtin_runner(Box::new(BotEventRouterRunner::new(
        1,
        vec![command_subscription()],
    )));
    bootstrapper.register_manifest(bot_command_manifest(1));
    bootstrapper.register_builtin_runner(Box::new(BotCommandRunner::new(1, vec!["/".into()])));
    bootstrapper.register_manifest(plugin::manifest(1));
    bootstrapper.register_builtin_runner(plugin::runner(1));
    bootstrapper.register_manifest(mock_transport::manifest(1));
    bootstrapper.register_builtin_runner(mock_transport::runner(1));
    bootstrapper.into_runtime(RuntimeProfile {
        profile_id: "template-dev".into(),
        mode: RuntimeProfileMode::FullDev,
        enabled_plugins: vec![
            mutsuki_plugin_bot_event_router::BOT_EVENT_ROUTER_PLUGIN_ID.into(),
            mutsuki_plugin_bot_command::BOT_COMMAND_PLUGIN_ID.into(),
            plugin::BUSINESS_PLUGIN_ID.into(),
            mock_transport::MOCK_TRANSPORT_PLUGIN_ID.into(),
        ],
        bindings: BTreeMap::new(),
        plugin_deployments: BTreeMap::new(),
        allow_dynamic_registration: false,
        allow_hot_reload: false,
    })
}

pub fn command_subscription() -> BotEventSubscription {
    BotEventSubscription {
        subscription_id: "message-to-command".into(),
        handler_protocol_id: BOT_COMMAND_PARSE_PROTOCOL_ID.into(),
        handler_binding_id: None,
        platform: None,
        event_kind: Some(BotEventKind::MessageCreated),
    }
}

fn default_secret_key() -> String {
    "QQBOT_CLIENT_SECRET".into()
}

fn set_if_some<T>(target: &mut T, value: Option<T>) {
    if let Some(value) = value {
        *target = value;
    }
}

#[cfg(test)]
mod profile_tests {
    use super::*;

    #[test]
    fn example_profile_maps_transport_and_gateway_settings_to_adapter_config() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("config")
            .join("qqbot.example.toml");
        let profile = QqBotProfile::load(&path).unwrap();

        let config = profile.adapter_config();

        assert_eq!(config.gateway_intents, 1_325_405_185);
        assert_eq!(config.request_timeout_ms, 15_000);
        assert_eq!(config.connect_timeout_ms, 10_000);
        assert_eq!(config.response_body_limit_bytes, 2_097_152);
        assert_eq!(config.reconnect_initial_delay_ms, 500);
        assert_eq!(config.reconnect_max_delay_ms, 30_000);
        assert_eq!(config.shard, [0, 1]);
        assert!(config.validate().is_ok());
    }
}
