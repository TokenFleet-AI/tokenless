# Specs

Design specifications and technical documentation for tokenless.

| # | Document | Description | 状态 |
|---|----------|-------------|------|
| 0001 | [Architecture Design](./0001-architecture.md) | 项目架构总览：Crate 规格、依赖关系、实现顺序 | ✅ 已更新至 v0.3.0 |
| 0002 | [Schema Compressor Enhancements](./0002-schema-compressor-enhancements.md) | Schema 压缩器三项增强：enum 截断、token-aware 截断、$ref 递归 | ✅ 已实现 |
| 0003 | [Data Flow & Pipeline Design](./0003-data-flow-pipeline-design.md) | 多阶段压缩管道与端到端数据流 | ✅ 已更新至 v0.3.0 |
| 0004 | [Hook Protocol Specification](./0004-hook-protocol-spec.md) | 11 种 Agent Hook 协议完整规范 | ✅ 已完成 |
| 0005 | [Security Model](./0005-security-model-design.md) | 威胁模型、信任边界与输入验证 | ✅ 已完成 |
| 0006 | [Error Handling Strategy](./0006-error-handling-strategy.md) | 错误处理策略与优雅降级模式 | ✅ 已完成 |
| 0007 | [Testing Strategy](./0007-testing-strategy.md) | 测试架构、覆盖分析与改进建议 | ✅ 已完成 |
| 0008 | [Deployment Architecture](./0008-deployment-architecture.md) | 构建管道、安装矩阵与 CI/CD | ✅ 已完成 |
| 0009 | [Optimization Analysis](./0009-optimization-analysis.md) | 性能优化机会与代码改进建议 | ✅ 已标记已解决项 (v0.3.0) |
| 0010 | [Innovation Roadmap](./0010-innovation-roadmap.md) | 创新方向与技术路线图（含状态标注） | ✅ 已更新至 v0.3.0 |
| 0011 | [MCP Server](./0011-mcp-server.md) | MCP JSON-RPC Server 集成：7 个 Tool | ✅ 已实现 + 实现状态 |
| 0012 | [Format Router](./0012-format-router.md) | 智能格式路由：结构分析 + 3 种编码策略 | ✅ 已实现 + 实现状态 |
| 0013 | [Differential Response](./0013-differential-response.md) | 差分响应压缩：轮询场景 unified diff | ✅ 已完成 |
| 0014 | [Semantic-Aware Compression](./0014-semantic-aware-compression.md) | 语义感知压缩：三级架构（规则→ONNX→远程 API） | 📝 Spec 完成 |
| 0015 | [Security Hardening](./0015-security-hardening.md) | 安全加固：9 项安全配置缺口修复（基于 rust-tui-template 审计） | ✅ 已完成 |
| 0016 | [Architecture Alignment](./0016-architecture-alignment.md) | 架构对齐：workspace 重组、lint 严格化、CLI 模块化、测试基础设施 | ✅ 已完成 |
| 0017 | [Stats History Management](./0017-stats-management.md) | 统计数据历史管理：删除、导出、维护 | 📝 Spec 完成 → 🔜 待实施 |
| 0018 | [Compression Stats Reporting](./0018-compression-stats-reporting.md) | agent-proxy → tokenless 请求完成回传协议：消耗、定价、压缩统计 | 📝 Spec 完成 → 🔜 待实施 |
