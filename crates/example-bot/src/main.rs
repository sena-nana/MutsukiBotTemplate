use std::path::Path;

use example_bot::{QqBotProfile, assemble_real_service, mock_runtime};
use mutsuki_bot_protocol::{
    BOT_COMMAND_PARSE_PROTOCOL_ID, BotAccountRef, BotEvent, BotEventKind, BotMessage, BotPlatform,
    BotTarget,
};
use mutsuki_runtime_contracts::Task;
use mutsuki_service_config::{ConfigOverrides, ServiceConfig};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var("MUTSUKI_BOT_MODE").as_deref() == Ok("qqbot") {
        let service = ServiceConfig::load(ConfigOverrides::default())?;
        let profile_path = std::env::var("MUTSUKI_BOT_PROFILE")
            .unwrap_or_else(|_| "config/qqbot.example.toml".into());
        let profile = QqBotProfile::load(Path::new(&profile_path))?;
        assemble_real_service(service, &profile)?
            .start()
            .await?
            .run_foreground()
            .await?;
    } else {
        let target = BotTarget::User {
            user_id: "local-user".into(),
        };
        let event = BotEvent {
            event_id: "local-echo".into(),
            platform: BotPlatform::Custom("mock".into()),
            bot: BotAccountRef {
                account_id: "local".into(),
                platform: BotPlatform::Custom("mock".into()),
            },
            kind: BotEventKind::MessageCreated,
            time_ms: 0,
            target: target.clone(),
            actor: None,
            message: Some(BotMessage::text(target, "/echo hello")),
            raw: None,
            ext: Default::default(),
        };
        let mut runtime = mock_runtime()?;
        runtime.submit_task(Task::new(
            "local-command",
            BOT_COMMAND_PARSE_PROTOCOL_ID,
            serde_json::to_value(event)?,
        ))?;
        let report = runtime.run_until_idle(16)?;
        println!("mock bot completed {} tasks", report.completed_tasks);
    }
    Ok(())
}
