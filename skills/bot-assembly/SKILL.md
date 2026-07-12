---
name: bot-assembly
description: Assemble Mutsuki BotTemplate from external configuration through plugin selection, RuntimeProfile, RuntimeLoadPlan, ServiceRuntimeBuilder, EventSources, and secret references. Use for startup, configuration, and product composition rather than upstream capability implementation.
---

# Bot Assembly

将外部配置确定性地转换为可验证的 Bot 产品装配，模板只描述所需能力。

## 配置

- 使用被 Git 忽略的外部配置；只提交中立 schema、字段契约、解析类型或生成逻辑。
- 配置只保存 secret key，由 Host 注入真实值。
- 未知字段和缺失 capability、plugin、deployment 或 secret 必须结构化失败。

## 装配

1. 将配置解析为 capability、plugin、deployment、binding、subscription 和 Host 资源需求。
2. 通过上游公开 builder、bundle、manifest 和 Runner factory 接入真实能力。
3. 启动前生成并校验 RuntimeProfile/RuntimeLoadPlan；registry freeze 后不得越权注册。
4. 通过 `ServiceRuntimeBuilder` 或当前等价 API 启动真实 `ServiceRuntime`，不创建 BotHost。

QQBot、Agent 和 Provider 只是配置选择与验收场景，不得产生绕过 Bot protocol 或上游公开边界的专用路径。测试合法配置的 load plan，并验证缺失项 fail loud、health 只报告真实组件。
