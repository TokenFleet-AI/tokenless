# Documentation Index

This directory contains reusable project documentation for the template repository.

## Tokenless 文档

- [SDK 集成指南 (英文)](./sdk-integration.md) — 第三方 Rust 库集成文档
- [SDK 集成指南 (中文)](./sdk-integration-zh.md) — 中文版 SDK 集成文档

- [用户指南与教程 (英文)](./user-guide.md) — 全面的使用教程、设计说明和 API 参考
- [用户指南与教程 (中文)](./user-guide-zh.md) — 中文版使用教程、设计说明和 API 参考

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
- [0011 MCP Server](../specs/0011-mcp-server.md)
- [0012 智能格式路由](../specs/0012-format-router.md)
- [0013 差分响应压缩](../specs/0013-differential-response.md)
- [0014 语义感知压缩](../specs/0014-semantic-aware-compression.md)
- [0015 安全加固](../specs/0015-security-hardening.md)
- [0016 架构对齐](../specs/0016-architecture-alignment.md)
- [0017 统计历史管理](../specs/0017-stats-management.md)
- [0018 压缩统计回传](../specs/0018-compression-stats-reporting.md)
- [0019 异步语义 Provider](../specs/0019-async-semantic-provider.md)
- [0020 团队成本控制台](../specs/0020-team-dashboard.md)
- [0021 策略配置中心](../specs/0021-policy-config-center.md)
- [Codex CLI 支持](../specs/codex-support-spec.md)

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

## 营销与推广

- [微头条引流文案](./marketing/weitoutao-copies.md) — 人设定位、5 种文案模板、发布策略

## 推荐阅读顺序

1. 先读 [SPARC 使用规范](./sparc-usage-guideline.md)，建立整体判断框架。
2. 再看 [提示词模板库](./prompt-template-library.md)，拿到可直接复制的任务模板。
3. 涉及测试先行时，补充阅读 [TDD 规范](./tdd-guideline.md)。
4. 涉及重构、安全、兼容性等高风险改动时，补充阅读 [高风险任务处理规范](./high-risk-task-guideline.md)。
5. 需要快速定位主题时，使用 [文档搜索索引](./search.md)。

## 项目健康分析

- [多角色并行分析综合规划](./analysis/multi-role-feature-plan-20260617.md) — 30 项功能规划 + 三阶段路线图
- [架构评审](./analysis/architecture-review-20260617.md) — Crate 边界、依赖图、扩展性分析
- [安全评审](./analysis/security-review-20260617.md) — 威胁矩阵、输入信任边界、供应链分析
- [性能评审](./analysis/performance-review-20260617.md) — 热路径分析、缓存效率、基准测试计划
- [UX/DevRel 评审](./analysis/ux-devrel-review-20260617.md) — 安装体验、文档完整性、传播能力
- [市场/创新评审](./analysis/market-innovation-review-20260617.md) — 竞品分析、商业模式、创新方向
- [代码质量评审](./analysis/code-quality-review-20260617.md) — 测试覆盖率、lint 纪律、CI/CD 健康度
- [多维度分析报告 (旧版)](./analysis/multi-dimensional-analysis.md) — 架构、文档、安全、性能四维度综合评估
- [性能基准报告](./performance-benchmarks.md) — 实际压缩效果与构建指标

## 参考指南

- [Agent 能力矩阵](./agent-capability-matrix.md) — 13 种 Agent 的压缩能力对照表
- [使用示例与 Recipes](./examples/recipes.md) — 场景化安装与使用示例

## 研究分析

- [模板迁移分析](./research/template-migration-analysis.md) — rust-tui-template 模板替换评估（RuFlo 6 Agent Swarm 联合审计）
- [WASM 构建可行性](./research/wasm-build-feasibility.md) — `tokenless-schema` 的 WebAssembly 构建能力、替代方案与后续计划

Owner: baoyx · 版本：v1.3 · 生效日期：2026-06-17 · 最后更新：2026-06-17
- 发布流程参考 [发布指南](./release-guide.md)。
