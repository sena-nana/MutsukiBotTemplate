use std::collections::BTreeMap;

use mutsuki_bot_protocol::{
    BOT_COMMAND_HANDLE_PROTOCOL_ID, BOT_MESSAGE_SEND_PROTOCOL_ID, BotCommandEvent, BotTarget,
};
use mutsuki_bot_sdk::MessageBuilder;
use mutsuki_runtime_contracts::{
    CompletionBatch, ExecutionClass, OrderingRequirement, PluginManifest, RunnerBatchCapability,
    RunnerControlCapability, RunnerDescriptor, RunnerMode, RunnerOrderingCapability,
    RunnerPayloadCapability, RunnerPurity, RunnerResourceCapability, RunnerResult,
    RunnerSideEffect, RuntimeError, ScalarValue, Task, WorkBatch,
};
use mutsuki_runtime_core::{Runner, RunnerContext, RuntimeResult};
use mutsuki_runtime_sdk::{PluginBuilder, map_work_batch_entries};
use serde_json::json;

#[cfg(feature = "agent-bot")]
use mutsuki_agent_protocol::{
    AGENT_RUN_PROTOCOL, AgentMessage, AgentModelResultCallback, AgentRunRequest,
};
#[cfg(feature = "agent-bot")]
use serde::{Deserialize, Serialize};

use crate::commands;

pub const BUSINESS_PLUGIN_ID: &str = "template.example_bot.business";
pub const BUSINESS_RUNNER_ID: &str = "template.example_bot.command";
#[cfg(feature = "agent-bot")]
pub const AGENT_RESULT_PROTOCOL_ID: &str = "template.agent.result/handle@1";

pub fn manifest(generation: u64) -> PluginManifest {
    PluginBuilder::new(BUSINESS_PLUGIN_ID)
        .runner_descriptor(descriptor(generation))
        .build()
        .manifest
}

pub fn runner(generation: u64) -> Box<dyn Runner> {
    Box::new(BusinessRunner::new(generation))
}

pub struct BusinessRunner {
    descriptor: RunnerDescriptor,
}

impl BusinessRunner {
    pub fn new(generation: u64) -> Self {
        Self {
            descriptor: descriptor(generation),
        }
    }
}

impl Runner for BusinessRunner {
    fn descriptor(&self) -> &RunnerDescriptor {
        &self.descriptor
    }

    fn run_batch(
        &mut self,
        ctx: RunnerContext,
        batch: WorkBatch,
    ) -> RuntimeResult<CompletionBatch> {
        map_work_batch_entries(&batch, |task| match task.protocol_id.as_str() {
            BOT_COMMAND_HANDLE_PROTOCOL_ID => handle_command(task, ctx.registry_generation),
            #[cfg(feature = "agent-bot")]
            AGENT_RESULT_PROTOCOL_ID => handle_agent_result(task, ctx.registry_generation),
            _ => Err(failure(format!(
                "unsupported.protocol:{}",
                task.protocol_id
            ))),
        })
    }
}

pub fn descriptor(generation: u64) -> RunnerDescriptor {
    RunnerDescriptor {
        runner_id: BUSINESS_RUNNER_ID.into(),
        plugin_id: BUSINESS_PLUGIN_ID.into(),
        plugin_generation: generation,
        accepted_protocol_ids: accepted_protocols(),
        purity: RunnerPurity::Pure,
        execution_class: ExecutionClass::Orchestration,
        input_schema: json!({"type": "object", "required": ["source", "name", "args"]}),
        output_schema: json!({"tasks": output_protocols()}),
        batch: RunnerBatchCapability {
            mode: RunnerMode::NativeBatch,
            preferred_batch_size: 16,
            max_batch_entries: 64,
            side_effect: RunnerSideEffect::None,
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
            ScalarValue::String("Template ping and echo business runner".into()),
        )]),
        contract_surfaces: accepted_protocols()
            .into_iter()
            .map(|protocol| format!("task_protocol:{protocol}"))
            .chain(std::iter::once(format!("runner:{BUSINESS_RUNNER_ID}")))
            .collect(),
    }
}

#[cfg(not(feature = "agent-bot"))]
fn accepted_protocols() -> Vec<String> {
    vec![BOT_COMMAND_HANDLE_PROTOCOL_ID.into()]
}
#[cfg(feature = "agent-bot")]
fn accepted_protocols() -> Vec<String> {
    vec![
        BOT_COMMAND_HANDLE_PROTOCOL_ID.into(),
        AGENT_RESULT_PROTOCOL_ID.into(),
    ]
}

#[cfg(not(feature = "agent-bot"))]
fn output_protocols() -> Vec<&'static str> {
    vec![BOT_MESSAGE_SEND_PROTOCOL_ID]
}
#[cfg(feature = "agent-bot")]
fn output_protocols() -> Vec<&'static str> {
    vec![BOT_MESSAGE_SEND_PROTOCOL_ID, AGENT_RUN_PROTOCOL]
}

fn handle_command(task: &Task, registry_generation: u64) -> Result<RunnerResult, RuntimeError> {
    let command: BotCommandEvent = serde_json::from_value(task.payload.clone())
        .map_err(|error| failure(format!("command.decode:{error}")))?;
    let source_message_id = command
        .source
        .message
        .as_ref()
        .and_then(|message| message.message_id.clone());
    let mut result = RunnerResult::completed(task.task_id.clone());
    #[cfg(feature = "agent-bot")]
    if matches!(command.name.as_str(), "ask" | "ask-tool") {
        result.tasks.push(agent_task(
            task,
            registry_generation,
            &command,
            source_message_id,
        )?);
        return Ok(result);
    }
    if let Some(text) = commands::reply(&command.name, &command.args) {
        result.tasks.push(send_task(
            task,
            registry_generation,
            command.source.event_id.clone(),
            command.source.target.clone(),
            source_message_id.clone(),
            text,
        )?);
    }
    Ok(result)
}

#[cfg(feature = "agent-bot")]
#[derive(Clone, Debug, Serialize, Deserialize)]
struct AgentReplyContext {
    source_event_id: String,
    target: BotTarget,
    reply_to: Option<String>,
    session_id: String,
}

#[cfg(feature = "agent-bot")]
fn agent_task(
    parent: &Task,
    registry_generation: u64,
    command: &BotCommandEvent,
    reply_to: Option<String>,
) -> Result<Task, RuntimeError> {
    let question = command.args.join(" ");
    let session_id = format!(
        "bot:{}",
        serde_json::to_string(&command.source.target)
            .map_err(|error| failure(format!("target.encode:{error}")))?
    );
    let context = AgentReplyContext {
        source_event_id: command.source.event_id.clone(),
        target: command.source.target.clone(),
        reply_to,
        session_id: session_id.clone(),
    };
    let mut request =
        AgentRunRequest::new("template-agent", vec![AgentMessage::user(question.clone())]);
    request.session_id = Some(session_id);
    request.result_protocol_id = Some(AGENT_RESULT_PROTOCOL_ID.into());
    request.result_context = Some(
        serde_json::to_value(context)
            .map_err(|error| failure(format!("agent.context.encode:{error}")))?,
    );
    if command.name == "ask-tool" {
        request.metadata = Some(json!({"tool": {"name": "echo", "input": {"question": question}}}));
    }
    let mut child = Task::new(
        format!("template.example_bot.agent:{}", command.source.event_id),
        AGENT_RUN_PROTOCOL,
        serde_json::to_value(request)
            .map_err(|error| failure(format!("agent.request.encode:{error}")))?,
    );
    inherit_task_context(parent, &mut child, registry_generation);
    Ok(child)
}

#[cfg(feature = "agent-bot")]
fn handle_agent_result(
    task: &Task,
    registry_generation: u64,
) -> Result<RunnerResult, RuntimeError> {
    let callback: AgentModelResultCallback = serde_json::from_value(task.payload.clone())
        .map_err(|error| failure(format!("agent.result.decode:{error}")))?;
    let context: AgentReplyContext = serde_json::from_value(
        callback
            .context
            .ok_or_else(|| failure("agent.result.context_missing"))?,
    )
    .map_err(|error| failure(format!("agent.context.decode:{error}")))?;
    if callback.session_id.as_deref() != Some(context.session_id.as_str()) {
        return Err(failure("agent.result.session_mismatch"));
    }
    let mut result = RunnerResult::completed(task.task_id.clone());
    result.tasks.push(send_task(
        task,
        registry_generation,
        context.source_event_id,
        context.target,
        context.reply_to,
        callback.result.message.content,
    )?);
    Ok(result)
}

fn send_task(
    parent: &Task,
    registry_generation: u64,
    source_event_id: String,
    target: BotTarget,
    reply_to: Option<String>,
    text: String,
) -> Result<Task, RuntimeError> {
    let mut message = MessageBuilder::new(target).text(text);
    if let Some(message_id) = reply_to {
        message = message.reply_to(message_id);
    }
    let mut send = Task::new(
        format!("template.example_bot.send:{source_event_id}"),
        BOT_MESSAGE_SEND_PROTOCOL_ID,
        serde_json::to_value(message.build())
            .map_err(|error| failure(format!("message.encode:{error}")))?,
    );
    inherit_task_context(parent, &mut send, registry_generation);
    Ok(send)
}

fn inherit_task_context(parent: &Task, child: &mut Task, registry_generation: u64) {
    child.registry_generation = registry_generation;
    child.trace_id = parent.trace_id.clone();
    child.correlation_id = parent.correlation_id.clone();
}

fn failure(route: impl Into<String>) -> RuntimeError {
    RuntimeError::new(
        mutsuki_runtime_contracts::ERR_RUNTIME_HOST_FAILED,
        BUSINESS_PLUGIN_ID,
        route,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use mutsuki_bot_protocol::{
        BotAccountRef, BotEvent, BotEventKind, BotMessage, BotPlatform, BotTarget,
    };
    use mutsuki_runtime_contracts::{BatchEntry, BatchPayload, DispatchLane, WorkResourcePlan};

    #[test]
    fn single_and_multi_entry_batches_preserve_context() {
        let mut runner = BusinessRunner::new(1);
        let completion = runner
            .run_batch(
                context(9, 2),
                batch(vec![
                    command_task("a", "echo", &["hello", "world"]),
                    command_task("b", "ping", &[]),
                ]),
            )
            .unwrap();

        assert_eq!(completion.results.len(), 2);
        let first = &completion.results[0].result.as_ref().unwrap().tasks[0];
        let second = &completion.results[1].result.as_ref().unwrap().tasks[0];
        assert_eq!(first.protocol_id, BOT_MESSAGE_SEND_PROTOCOL_ID);
        assert_eq!(first.payload["segments"][0]["text"], "hello world");
        assert_eq!(first.payload["reply_to"], "message-a");
        assert_eq!(second.payload["segments"][0]["text"], "pong");
        assert_eq!(first.trace_id.as_deref(), Some("trace-a"));
        assert_eq!(first.registry_generation, 9);
    }

    #[test]
    fn decode_failure_only_fails_its_entry() {
        let mut runner = BusinessRunner::new(1);
        let invalid = Task::new("bad", BOT_COMMAND_HANDLE_PROTOCOL_ID, json!({}));
        let completion = runner
            .run_batch(
                context(3, 3),
                batch(vec![
                    command_task("a", "ping", &[]),
                    invalid,
                    command_task("c", "echo", &["ok"]),
                ]),
            )
            .unwrap();

        assert!(completion.results[0].result.is_some());
        assert!(completion.results[1].error.is_some());
        assert!(completion.results[2].result.is_some());
    }

    fn command_task(id: &str, name: &str, args: &[&str]) -> Task {
        let target = BotTarget::User {
            user_id: "user".into(),
        };
        let mut source_message = BotMessage::text(target.clone(), format!("/{name}"));
        source_message.message_id = Some(format!("message-{id}"));
        let source = BotEvent {
            event_id: format!("event-{id}"),
            platform: BotPlatform::Custom("test".into()),
            bot: BotAccountRef {
                account_id: "bot".into(),
                platform: BotPlatform::Custom("test".into()),
            },
            kind: BotEventKind::MessageCreated,
            time_ms: 1,
            target: target.clone(),
            actor: None,
            message: Some(source_message),
            raw: None,
            ext: Default::default(),
        };
        let mut task = Task::new(
            id,
            BOT_COMMAND_HANDLE_PROTOCOL_ID,
            serde_json::to_value(BotCommandEvent {
                source,
                name: name.into(),
                args: args.iter().map(|arg| (*arg).into()).collect(),
                raw_text: format!("/{name}"),
            })
            .unwrap(),
        );
        task.trace_id = Some(format!("trace-{id}"));
        task
    }

    fn batch(tasks: Vec<Task>) -> WorkBatch {
        WorkBatch {
            batch_id: "business-batch".into(),
            tick_id: "tick".into(),
            batch_key: BUSINESS_RUNNER_ID.into(),
            entries: tasks
                .iter()
                .enumerate()
                .map(|(index, task)| BatchEntry {
                    entry_id: format!("entry-{index}"),
                    task_id: task.task_id.clone(),
                    trace_id: task.trace_id.clone(),
                    parent_id: None,
                    payload_index: index,
                    resource_requirement_indices: Vec::new(),
                    cancel_index: None,
                    deadline_tick: None,
                    priority: 0,
                    lane: DispatchLane::Normal,
                    ordering: OrderingRequirement::PreserveSubmitOrder,
                })
                .collect(),
            payload: BatchPayload::from_tasks(&tasks),
            resource_plan: WorkResourcePlan::empty(),
            task_leases: Vec::new(),
        }
    }

    fn context(generation: u64, count: usize) -> RunnerContext {
        RunnerContext::new(
            generation,
            1,
            "executor",
            Vec::<String>::new(),
            "business-batch",
        )
        .with_batch("business-batch", count)
    }
}
