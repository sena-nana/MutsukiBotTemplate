# MutsukiBotTemplate

配置驱动、实现中立的 Mutsuki Bot 产品模板。生产 crate 只链接通用 Bot 协议、最小业务
Runner 和 ServiceHost 装配入口；平台 Adapter、Agent、Provider 与 transport 由使用方的外部
ServiceHost 配置和插件 catalog 选择。

## Run

配置文件不属于模板仓库。创建本地 ServiceHost 配置后，将路径作为唯一参数传入：

```powershell
cargo run -p example-bot -- path/to/local-service.toml
```

配置通过 `[[plugins.configured]]` 选择链接进产品 catalog 的原生插件，并可继续声明外部
artifact/deployment。缺失 factory、capability 或 secret key 在启动阶段结构化失败；模板不会
切换到 mock、空 Adapter 或默认 Provider。

QQ 文本 Bot 至少选择以下 owner 插件 ID：

```toml
[[plugins.configured]] # config: subscriptions
id = "mutsuki.bot.router.event"

[[plugins.configured]] # config: prefixes
id = "mutsuki.bot.command"

[[plugins.configured]] # config: account/app/network/client_secret_key
id = "mutsuki.bot.adapter.qqbot"
```

这是结构片段，不是可运行配置。QQ 字段由 BotPlugins 严格解析；模板只注册其 factory
catalog。`client_secret_key = "QQBOT_CLIENT_SECRET"` 对应默认 Host 环境变量
`MUTSUKI_SECRET_QQBOT_CLIENT_SECRET`，配置中不得保存 secret 或 access token。

## Business Runner

`example-bot` 固定注册的只有 `template.example_bot.business`。它消费通用 Bot command task，
并产生 `mutsuki.bot.message/send@1`；平台路由、命令解析和消息发送由配置选中的 owner
factory 提供。QQ 默认 factory 是文本模式，不虚假声明媒体能力。

## QQ verification

自动 E2E 使用 BotPlugins 的真实本地 HTTP/WebSocket fake，保留完整 ServiceRuntime、
EventSource、Runner 和 task routing：

```powershell
cargo test -p example-bot --test qqbot_config_e2e
```

真实账号 smoke 需要 ignored local config 和对应 Host secret；启动后在群内发送 `/ping` 与
`/echo hello`：

```powershell
$env:MUTSUKI_QQBOT_SMOKE_CONFIG = "path/to/local-service.toml"
cargo test -p example-bot --test qqbot_real_smoke -- --ignored --nocapture
```

未提供本地凭据时该层不执行，也不能用 fake 结果替代。

## Verification

```powershell
cargo metadata --locked
cargo fmt --check
cargo check
cargo test
cargo test -p example-bot --features agent-bot
```

测试在临时目录生成配置。QQBot 与 Agent 验收使用上游公开 integration/bundle，并只替换
外部平台或 Provider 边界。

跨仓库职责见 [docs/repository-boundaries.md](docs/repository-boundaries.md)。
