# MutsukiBotTemplate

配置驱动、实现中立的 Mutsuki Bot 产品模板。生产 crate 只链接通用 Bot 协议、最小业务
Runner 和 ServiceHost 装配入口；平台 Adapter、Agent、Provider 与 transport 由使用方的外部
ServiceHost 配置和插件 catalog 选择。

## Run

配置文件不属于模板仓库。创建本地 ServiceHost 配置后，将路径作为唯一参数传入：

```powershell
cargo run -p example-bot -- path/to/local-service.toml
```

配置必须声明真实插件、artifact、deployment、capability 和 secret key。缺失项在启动阶段
结构化失败；模板不会切换到 mock、空 Adapter 或默认 Provider。

## Business Runner

`example-bot` 只注册 `template.example_bot.business`。它消费通用 Bot command task，并产生
`mutsuki.bot.message/send@1`；平台路由、命令解析和消息发送实现均由外部插件提供。

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
