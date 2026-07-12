use mutsuki_agent_protocol::{AgentToolDescriptor, ToolSideEffect};
use mutsuki_agent_sdk::orchestration_runner;
use mutsuki_runtime_contracts::{
    CompletionBatch, DomainEvent, PluginManifest, RunnerDescriptor, RunnerResult, Task, WorkBatch,
};
use mutsuki_runtime_core::{Runner, RunnerContext, RuntimeResult};
use mutsuki_runtime_sdk::{PluginBuilder, map_work_batch_entries};

pub const TOOL_PLUGIN_ID: &str = "template.agent.tool.echo";
pub const TOOL_RUNNER_ID: &str = "template.agent.tool.echo.runner";
pub const TOOL_PROTOCOL_ID: &str = "template.agent.tool/echo@1";

pub fn tool_descriptor() -> AgentToolDescriptor {
    let mut descriptor = AgentToolDescriptor::new(
        "echo",
        TOOL_PROTOCOL_ID,
        "Returns input without side effects",
    );
    descriptor.side_effect = ToolSideEffect::None;
    descriptor
}

pub fn manifest(generation: u64) -> PluginManifest {
    PluginBuilder::new(TOOL_PLUGIN_ID)
        .runner_descriptor(descriptor(generation))
        .build()
        .manifest
}

pub fn runner(generation: u64) -> Box<dyn Runner> {
    Box::new(EchoToolRunner {
        descriptor: descriptor(generation),
    })
}

fn descriptor(generation: u64) -> RunnerDescriptor {
    orchestration_runner(TOOL_RUNNER_ID, TOOL_PLUGIN_ID)
        .plugin_generation(generation)
        .accepted_protocol(TOOL_PROTOCOL_ID)
        .build()
}

struct EchoToolRunner {
    descriptor: RunnerDescriptor,
}

impl Runner for EchoToolRunner {
    fn descriptor(&self) -> &RunnerDescriptor {
        &self.descriptor
    }

    fn run_batch(
        &mut self,
        _ctx: RunnerContext,
        batch: WorkBatch,
    ) -> RuntimeResult<CompletionBatch> {
        map_work_batch_entries(&batch, |task| Ok(echo(task)))
    }
}

fn echo(task: &Task) -> RunnerResult {
    let mut result = RunnerResult::completed(task.task_id.clone());
    result.events.push(DomainEvent {
        event_id: format!("{}:result", task.task_id),
        kind: "template.agent.tool.echo.completed".into(),
        payload: task.payload.clone(),
    });
    result
}
