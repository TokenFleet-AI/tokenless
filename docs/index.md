# Documentation Index

This directory contains reusable project documentation for the template repository.

## Tokenless 文档

- [用户指南与教程](./user-guide.md) — 全面的使用教程、设计说明和 API 参考

- [Tokenless 架构设计 (中文)](./design/tokenless-architecture-zh.md) — 中文版架构设计文档
- [Architecture (EN)](../specs/0001-architecture.md) — 英文版架构规格（Specs 0001）

## 设计规格（Specs）

详见 [specs/index.md](../specs/index.md)

- [0001 架构设计](../specs/0001-architecture.md)
- [0002 Schema Compressor 增强](../specs/0002-schema-compressor-enhancements.md)
- [0003 数据流与管道设计](../specs/0003-data-flow-pipeline-design.md)
- [0004 Hook 协议规范](../specs/0004-hook-protocol-spec.md)
- [0005 安全模型设计](../specs/0005-security-model-design.md)
- [0006 错误处理策略](../specs/0006-error-handling-strategy.md)
- [0007 测试策略](../specs/0007-testing-strategy.md)
- [0008 部署架构](../specs/0008-deployment-architecture.md)
- [0009 优化分析](../specs/0009-optimization-analysis.md)
- [0010 创新路线图](../specs/0010-innovation-roadmap.md)

## Agent workflow

- [Ruflo Usage](./ruflo-usage.md) — how this template uses Ruflo for agent workflow and orchestration.
- [CodeGraph Usage](./codegraph-usage.md) — 通用代码图谱/关系分析教程，用于快速理解仓库结构、调用链和影响面。

## Development workflow

- [Pre-commit Usage](./pre-commit-usage.md) — how to install and run repository pre-commit hooks.

## SPARC 文档中心

小任务找专家，大任务找协调器；`TDD` 是规则，不是入口；高风险任务不得单 Agent 一把梭。

- [SPARC 使用规范](./sparc-usage-guideline.md) — 内部使用规范，用于统一单 Agent 与多智能体工作流的入口选择。
- [提示词模板库](./prompt-template-library.md) — 常用 SPARC 任务提示词模板，可直接复制使用。
- [TDD 规范](./tdd-guideline.md) — TDD 工作流规则、推荐顺序与阶段门禁。
- [高风险任务处理规范](./high-risk-task-guideline.md) — 高风险改动的协作、测试与审查要求。
- [文档搜索索引](./search.md) — GitHub 可渲染的轻量检索入口。

## 推荐阅读顺序

1. 先读 [SPARC 使用规范](./sparc-usage-guideline.md)，建立整体判断框架。
2. 再看 [提示词模板库](./prompt-template-library.md)，拿到可直接复制的任务模板。
3. 涉及测试先行时，补充阅读 [TDD 规范](./tdd-guideline.md)。
4. 涉及重构、安全、兼容性等高风险改动时，补充阅读 [高风险任务处理规范](./high-risk-task-guideline.md)。
5. 需要快速定位主题时，使用 [文档搜索索引](./search.md)。

Owner: baoyx · 版本：v1.0 · 生效日期：2026-05-21 · 最后更新：2026-05-21
