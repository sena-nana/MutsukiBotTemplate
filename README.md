# MutsukiBotTemplate

配置驱动、实现中立的 Mutsuki Bot 产品装配器。`mutsuki-bot` 只加载 ServiceHost 配置、
注册 owner 提供的插件 factory catalog 并启动 Runtime；它不实现命令、回复、Agent 流程或
任何具体业务 Bot。

## Run

创建本地配置和 Secret 文件：

```powershell
Copy-Item config/template.toml config/local.toml
Copy-Item config/secret.template.toml config/local.secret.toml
cargo run -p mutsuki-bot
```

配置路径优先级为命令行参数、`MUTSUKI_CONFIG`、`config/local.toml`：

```powershell
cargo run -p mutsuki-bot -- path/to/product.toml
```

提交的 `config/template.toml` 是零插件的中立产品。零插件 Runtime 可以启动和停止，但没有
平台连接或业务行为。最终产品从已经链接或安装的 owner 插件中选择能力：

```toml
[[plugins.configured]]
id = "owner.plugin.id"

[plugins.configured.config]
# 仅由插件 owner 定义和解析
```

Native factory 必须由对应依赖仓库链接进 catalog；外部插件 artifact 由 ServiceHost 插件目录
发现。新增业务能力应在 BotPlugins、AgentKit 或独立业务仓库实现并发布，产品只修改配置进行
选择，禁止把业务 Runner 复制到本模板。

主配置只保存 Secret key 引用，实际值放在被 Git 忽略的 `config/local.secret.toml`，或使用
`MUTSUKI_SECRET_<KEY>` 环境变量覆盖。默认 Runtime home 是 `~/.mutsuki`，其下包含
`data`、`logs`、`plugins` 和 `run`。

## QQ Gateway smoke

QQ 只是可选平台插件和验收场景。自动测试使用 BotPlugins fake 验证配置装配、Gateway、
health、Resume 和 graceful shutdown，不包含命令或回复：

```powershell
cargo test -p mutsuki-bot --test qqbot_config_e2e
```

真实账号 smoke 只验证鉴权、Gateway 连接和 health：

```powershell
cargo test -p mutsuki-bot --test qqbot_real_smoke -- --ignored --nocapture
```

## Verification

```powershell
cargo metadata --locked
cargo fmt --check
cargo check --locked
cargo test --locked
```

跨仓库职责见 [docs/repository-boundaries.md](docs/repository-boundaries.md)。
