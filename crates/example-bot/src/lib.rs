pub mod commands;
pub mod plugin;

use mutsuki_service_config::ServiceConfig;
use mutsuki_service_runtime::ServiceRuntimeBuilder;

/// Assemble only the platform-neutral business plugin. Platform adapters,
/// routing, Agent providers and transports must be selected by external Host
/// configuration and registered by their owning plugin packages.
pub fn assemble_service(service: ServiceConfig) -> ServiceRuntimeBuilder {
    ServiceRuntimeBuilder::new(service)
        .register_builtin_plugin(plugin::manifest(1))
        .register_builtin_runner(|| plugin::runner(1))
}
