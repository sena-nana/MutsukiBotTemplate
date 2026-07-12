pub mod commands;
pub mod plugin;

use mutsuki_bot_service_host_integration::configured_bot_plugin_catalog;
use mutsuki_service_config::ServiceConfig;
use mutsuki_service_runtime::{ServiceRuntimeBuilder, ServiceRuntimeResult};

/// Assemble only the platform-neutral business plugin. Platform adapters,
/// routing, Agent providers and transports must be selected by external Host
/// configuration and registered by their owning plugin packages.
pub fn assemble_service(service: ServiceConfig) -> ServiceRuntimeResult<ServiceRuntimeBuilder> {
    Ok(ServiceRuntimeBuilder::new(service)
        .with_configured_plugin_catalog(configured_bot_plugin_catalog()?)
        .register_builtin_plugin(plugin::manifest(1))
        .register_builtin_runner(|| plugin::runner(1)))
}
