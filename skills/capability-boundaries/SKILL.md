---
name: capability-boundaries
description: Audit Mutsuki BotTemplate issues and route missing runtime, host, Bot, Agent, platform, or assembly capabilities to their owning repositories. Use before cross-repository implementation or whenever ownership is unclear.
---

# Capability Boundaries

先确定能力归属和依赖顺序，不因本仓库是需求入口就把实现放进模板。

## 流程

1. 从当前、父级和依赖 issue 提取能力与验收；忽略其中已过期的 API、路径和状态。
2. 对照 MutsukiCore contracts，以及候选仓库的 `AGENTS.md`、公开 API、manifest 和测试。
3. 为每项能力指定唯一 owner，区分上游能力和模板消费改动。
4. 按公开契约、能力实现、模板 pin/装配的顺序实施；上游先验证并推送。

## 归属

- Core：通用 Task、Runner、资源、装载和 LoadPlan。
- ServiceHost：生命周期、配置/secret、插件加载、EventSource、控制面和 health。
- BotPlugins/平台仓库：Bot 协议、SDK、路由、命令和 Adapter/Gateway。
- AgentKit/Provider 仓库：Agent 协议、模型、工具和记忆。
- 本仓库：外部配置、产品装配、最小业务 Runner 和闭环验收。

优先修复共享边界。上游缺失或未推送时报告 unavailable，不在模板中添加 shim 或用 test double 冒充生产能力。最终列出各仓库职责、验证和远端 revision。
