# MutsukiBotTemplate 工作规范

本仓库是 **配置驱动的 Mutsuki Bot 产品模板**，也是 Bot 需求进入多仓库体系后的
能力边界核查入口。它只负责外部配置契约、产品装配、最小业务示例和闭环验收，
不拥有 Core、Host、Bot、Agent 或平台能力的实现。

## 阅读顺序

1. 当前及关联 issue，确认目标、依赖和验收场景。
2. `../MutsukiCore/AGENTS.md` 与 `plans/{roadmap,architecture,engineering,contracts}.md`。
3. 候选依赖仓库的 `AGENTS.md`、公开 API、manifest 和测试。
4. 本文件路由的相关技能，再检查当前实现、远端 commit 和 lockfile。

Issue 是需求线索，不是当前 API 的事实源。存在 `.codegraph/` 时，定位代码先用 CodeGraph。

## 技能路由

- `skills/capability-boundaries/SKILL.md`：判断能力归属和跨仓库顺序。
- `skills/remote-dependencies/SKILL.md`：Git 依赖、远端 pin、lockfile 和独立 checkout。
- `skills/bot-assembly/SKILL.md`：配置契约、LoadPlan 和 ServiceRuntime 装配。
- `skills/business-runner/SKILL.md`：命令、Bot task 和 batch-first 业务 Runner。
- `skills/integration-testing/SKILL.md`：mock、fake server、真实 smoke、health 和 shutdown。

职责不明先读 capability-boundaries；涉及依赖同时读 remote-dependencies。

## 职责边界

| 仓库 | 职责 |
| --- | --- |
| `MutsukiCore` | 领域中立 contracts、Task/Runner、资源、LoadPlan 和 Rust Host/SDK 基础面 |
| `MutsukiStdPlugins` | 领域中立标准协议，以及 config/db/fs/http/observe/resource/workflow 插件 |
| `MutsukiPythonRunnerKit` | Core Runner Link 的 Python contract mirror、Runner backend、transport 和测试工具 |
| `MutsukiServiceHost` | 服务生命周期、配置/secret、插件加载、EventSource、控制面和 health |
| `MutsukiBotPlugins` | `mutsuki.bot.*` 协议、Bot SDK、标准 Runner、平台 Adapter/Gateway 和显式 Host integration crate |
| `MutsukiAgentKit` | Agent 协议、SDK、模型、工具和记忆能力 |
| `MutsukiCliHost` | ServiceHost 公开控制 API 的 CLI/TUI 客户端 |
| `MutsukiTauriHost` | 内嵌 Core 的桌面 Host、Tauri/WebView bridge 和前端 SDK |
| 其他能力仓库 | 自己领域的协议、插件、Provider、Runner 或 sidecar |
| 本仓库 | 外部配置契约、产品装配、最小业务示例和跨仓库闭环验收 |

## Hard Rules

1. 能力缺失时在所属仓库补齐、验证并推送，再更新模板；禁止复制实现、生产 fallback 或兼容 shim。
2. 禁止提交指向仓库外的 Cargo `path`/`[patch]`；跨仓库 Git 依赖固定 `rev`，且远端必须可独立解析。仓库内 member path 不受此限。
3. 配置只声明 capability、插件和部署选择。模板不按平台、Agent、Provider 或 backend 硬编码替代路径。
4. 不提交可运行配置、账号或 secret；只提交中立 schema、字段契约或生成逻辑。Secret 仅由 Host key 引用和注入。
5. Runner 只走 batch-first `run_batch`；task 操作使用 `TaskHandle`/`TaskSubmitter`；业务只依赖通用 Bot 协议。
6. RuntimeProfile/RuntimeLoadPlan 是装配权威；registry freeze 后不得动态越权注册。
7. 缺失 capability、配置、secret、artifact 或 revision 必须结构化失败，禁止假成功和吞错。
8. 生产入口必须要求外部配置路径，只固定注册平台中立业务 Runner；可用平台/Agent/Provider 只能作为 owner factory catalog 暴露，由配置选择，禁止默认启用或 fallback。Mock 仅限测试。

## Git 与验证

- 工作前后检查 `git status --short`；跨仓库先提交并推送上游，再提交模板 pin。
- Rust 或依赖改动运行 `cargo fmt --check`、`cargo check`、`cargo test`。
- 装配或依赖改动再运行 `cargo metadata --locked`，并在没有兄弟仓库的独立 checkout 验证。
- 最终说明列出实际命令和结果；测试断言行为，不只匹配日志或文案。
