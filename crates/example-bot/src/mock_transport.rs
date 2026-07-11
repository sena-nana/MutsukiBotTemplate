use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use mutsuki_bot_protocol::{BOT_MESSAGE_SEND_PROTOCOL_ID, BotMessage};
use mutsuki_runtime_contracts::{
    CompletionBatch, DomainEvent, ExecutionClass, OrderingRequirement, PluginManifest,
    RunnerBatchCapability, RunnerControlCapability, RunnerDescriptor, RunnerMode,
    RunnerOrderingCapability, RunnerPayloadCapability, RunnerPurity, RunnerResourceCapability,
    RunnerResult, RunnerSideEffect, RuntimeError, ScalarValue, WorkBatch,
};
use mutsuki_runtime_core::{Runner, RunnerContext, RuntimeResult};
use mutsuki_runtime_host::runner_manifest;
use mutsuki_runtime_sdk::map_work_batch_entries;
use serde_json::json;

pub const MOCK_TRANSPORT_PLUGIN_ID: &str = "template.mock.transport";
const MOCK_TRANSPORT_RUNNER_ID: &str = "template.mock.transport.send";

pub fn manifest(generation: u64) -> PluginManifest {
    runner_manifest(MOCK_TRANSPORT_PLUGIN_ID, vec![descriptor(generation)])
}

pub fn runner(generation: u64) -> Box<dyn Runner> {
    Box::new(MockTransportRunner {
        descriptor: descriptor(generation),
        recorded: None,
        notify: None,
        fail: false,
    })
}

pub fn recording_runner(
    generation: u64,
    recorded: Arc<Mutex<Vec<BotMessage>>>,
    notify: Arc<tokio::sync::Notify>,
) -> Box<dyn Runner> {
    Box::new(MockTransportRunner {
        descriptor: descriptor(generation),
        recorded: Some(recorded),
        notify: Some(notify),
        fail: false,
    })
}

pub fn failing_runner(generation: u64, notify: Arc<tokio::sync::Notify>) -> Box<dyn Runner> {
    Box::new(MockTransportRunner {
        descriptor: descriptor(generation),
        recorded: None,
        notify: Some(notify),
        fail: true,
    })
}

struct MockTransportRunner {
    descriptor: RunnerDescriptor,
    recorded: Option<Arc<Mutex<Vec<BotMessage>>>>,
    notify: Option<Arc<tokio::sync::Notify>>,
    fail: bool,
}

impl Runner for MockTransportRunner {
    fn descriptor(&self) -> &RunnerDescriptor {
        &self.descriptor
    }

    fn run_batch(
        &mut self,
        _ctx: RunnerContext,
        batch: WorkBatch,
    ) -> RuntimeResult<CompletionBatch> {
        map_work_batch_entries(&batch, |task| {
            let message: BotMessage = serde_json::from_value(task.payload.clone())
                .map_err(|error| failure(format!("message.decode:{error}")))?;
            if let Some(recorded) = &self.recorded {
                recorded
                    .lock()
                    .expect("mock transport mutex")
                    .push(message.clone());
            }
            if let Some(notify) = &self.notify {
                notify.notify_waiters();
            }
            if self.fail {
                return Err(failure("message.send.failed"));
            }
            let mut result = RunnerResult::completed(task.task_id.clone());
            result.events.push(DomainEvent {
                event_id: format!("{}:sent", task.task_id),
                kind: "template.mock.message.sent".into(),
                payload: serde_json::to_value(message)
                    .map_err(|error| failure(format!("message.encode:{error}")))?,
            });
            Ok(result)
        })
    }
}

fn descriptor(generation: u64) -> RunnerDescriptor {
    RunnerDescriptor {
        runner_id: MOCK_TRANSPORT_RUNNER_ID.into(),
        plugin_id: MOCK_TRANSPORT_PLUGIN_ID.into(),
        plugin_generation: generation,
        accepted_protocol_ids: vec![BOT_MESSAGE_SEND_PROTOCOL_ID.into()],
        purity: RunnerPurity::Pure,
        execution_class: ExecutionClass::Io,
        input_schema: json!({"type": "object", "required": ["target", "segments"]}),
        output_schema: json!({"events": ["template.mock.message.sent"]}),
        batch: RunnerBatchCapability {
            mode: RunnerMode::NativeBatch,
            preferred_batch_size: 16,
            max_batch_entries: 64,
            side_effect: RunnerSideEffect::External,
            ..Default::default()
        },
        payload: RunnerPayloadCapability::default(),
        resources: RunnerResourceCapability::default(),
        ordering: RunnerOrderingCapability {
            default: OrderingRequirement::PreserveSubmitOrder,
            supports_sequence: true,
            supports_same_resource_order: true,
        },
        control: RunnerControlCapability::default(),
        metadata: BTreeMap::from([(
            "description".into(),
            ScalarValue::String("Offline deterministic Bot transport".into()),
        )]),
        contract_surfaces: vec![format!("runner:{MOCK_TRANSPORT_RUNNER_ID}")],
    }
}

fn failure(route: impl Into<String>) -> RuntimeError {
    RuntimeError::new(
        mutsuki_runtime_contracts::ERR_RUNTIME_HOST_FAILED,
        MOCK_TRANSPORT_PLUGIN_ID,
        route,
    )
}
