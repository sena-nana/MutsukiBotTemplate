---
name: business-runner
description: Implement the template's minimal Bot commands and business Runner through current Mutsuki Bot protocols and batch-first contracts. Use for command behavior, manifests, TaskHandle submission, per-entry completion, context propagation, and protocol-only replies.
---

# Business Runner

只实现可复制的最小业务行为；平台、网络、Host 和 Agent 能力留在上游。

## 规则

- 只消费上游通用 `mutsuki.bot.*` DTO；回复生成通用 Bot task，由 Adapter/Gateway 执行外部效果。
- 不依赖平台 SDK，不持有 socket、HTTP client、Core handle、Host service 或平台对象。
- 只实现 batch-first `run_batch`；每个 BatchEntry 对应一个 EntryCompletion，单项失败不污染其他 entry。
- 继承 registry generation、trace/correlation 和 binding context；使用 `TaskSubmitter`、`RuntimeClient`、`TaskHandle` 等公开面。
- 让 manifest/RunnerDescriptor 的 protocol 和 capability 声明与实现一致。

## 测试

- 覆盖 single/multi-entry、局部 decode failure 和回复 task 的协议、payload、上下文与目标。
- 证明新增命令不需要修改 Core、ServiceHost、事件路由或平台 Adapter。
