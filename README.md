# MutsukiBotTemplate

可复制的 Rust-first Mutsuki Bot 模板。业务 crate 只消费 `mutsuki.bot.*` 协议；QQBot adapter 和 `MutsukiServiceHost` 只在产品装配入口出现。

## 离线运行

相邻目录需包含 `MutsukiCore`、`MutsukiServiceHost` 和 `MutsukiBotPlugins`：

```powershell
cargo run -p example-bot
cargo fmt --check
cargo check
cargo test
```

默认运行真实 batch-first command/business runner，不需要网络或凭据。

## QQBot 模式

复制 `config/qqbot.example.toml` 为被 `.gitignore` 排除的 `config/local.toml`，只填写非 secret 配置。Secret 通过 Host boundary 注入：

```powershell
$home = "$PWD/.mutsuki-qqbot"
Copy-Item config/qqbot.example.toml config/local.toml
# 编辑 config/local.toml，只填写 account_id、app_id、intents 和 secret key 名称。
$env:MUTSUKI_BOT_MODE = "qqbot"
$env:MUTSUKI_BOT_PROFILE = "config/local.toml"
$env:MUTSUKI_HOME = $home
$env:MUTSUKI_CONTROL_TOKEN = "dev-token"
$env:MUTSUKI_SECRET_QQBOT_CLIENT_SECRET = "your-secret"
cargo run -p example-bot
```

请把 `your-secret` 替换为真实 Client Secret，但不要把 Secret 写入 TOML、命令行参数、日志或聊天。
`QqBotPluginBundle` 会声明必需 Secret，由 ServiceHost 在启动 Core、IPC 和 EventSource 前完成预检。
`config/local.toml` 的 `[transport]` 与 `[gateway]` 可配置 API 地址、请求/连接超时、响应体上限、
token 刷新与重试、心跳/ACK 超时、有界队列、去重窗口、分片和指数重连策略；省略字段时使用 Adapter 安全默认值。

在另一个 PowerShell 窗口中查询同一实例：

```powershell
cargo run --manifest-path ../MutsukiServiceHost/Cargo.toml -p mutsuki-service-host -- --home .mutsuki-qqbot --token dev-token health
cargo run --manifest-path ../MutsukiServiceHost/Cargo.toml -p mutsuki-service-host -- --home .mutsuki-qqbot --token dev-token event-source list
```

`health.components["mutsuki.bot.qqbot.gateway:<account_id>"]` 会包含：

- `connected`
- `identified`
- `last_heartbeat_unix_ms`
- `last_ack_unix_ms`
- `last_event_unix_ms`
- `reconnect_count`
- `last_error`

### 真实账号验收

1. 在已启用的群或 C2C 会话中发送 `/echo hello`，确认回复 `hello`。
2. 如果账号只开放群 @ 消息，发送 `@机器人 /echo hello`，确认自身 mention 被 Adapter 移除后仍回复 `hello`。
3. 临时断开本机网络后恢复，重新执行 `health`，确认 Gateway 恢复为 `connected = true`、`identified = true`，且 `reconnect_count` 增加。
4. 执行停止命令：

```powershell
cargo run --manifest-path ../MutsukiServiceHost/Cargo.toml -p mutsuki-service-host -- --home .mutsuki-qqbot --token dev-token stop
```

5. 确认进程退出，WebSocket 已关闭，EventSource 状态停止且没有残留任务。

`ServiceRuntimeBuilder` 装配 event router、command parser、业务 runner、QQ adapter runners 与 Gateway EventSource；停止信号、health 和 source lease 由 ServiceHost 管理。

## 新增命令

在 `crates/example-bot/src/commands/` 添加纯函数，并在 `commands/mod.rs` 的 `reply` 分派中增加一项。无需修改 QQ adapter、Core 或 ServiceHost。项目名、crate 名、plugin id 和 runner id 集中在根 `Cargo.toml` 与 `plugin.rs`。
