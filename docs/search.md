# 文档搜索索引

GitHub 仓库浏览器不会执行原 HTML 搜索页中的前端 JavaScript。为了让内容在 GitHub 上直接可读，本页改为 Markdown 检索索引。

## 核心页面

| 页面 | 适合查找 | 链接 |
| --- | --- | --- |
| SPARC 使用规范 | 入口模式、专家模式、orchestrator、swarm-coordinator、TDD 规则、复杂任务、小任务、决策树、反模式、场景矩阵 | [打开](./sparc-usage-guideline.md) |
| 提示词模板库 | 提示词、模板、coder、tester、reviewer、orchestrator、swarm-coordinator、多智能体、TDD、补测试 | [打开](./prompt-template-library.md) |
| TDD 规范 | TDD、tester、coder、reviewer、failing tests、review clear、RED_CONFIRMED、TEST_PLAN_READY | [打开](./tdd-guideline.md) |
| 高风险任务处理规范 | 高风险、重构、安全、兼容性、公共库、reviewer、tester、orchestrator、swarm-coordinator | [打开](./high-risk-task-guideline.md) |
| CodeGraph 使用教程 | codegraph、@colbymchenry/codegraph、Claude Code、codegraph init -i、codegraph install、MCP、代码图谱、调用链、依赖分析、影响面、安装、初始化、更新、官方链接 | [打开](./codegraph-usage.md) |

## 推荐检索方式

- 在 GitHub 页面按 `t` 可以快速搜索仓库文件名。
- 在当前 Markdown 页面使用浏览器搜索，输入关键词如 `TDD`、`review`、`swarm`、`高风险`。
- 在本地仓库中使用 `rg "关键词" docs` 检索全部文档内容。

## 导航

- 上一篇：[高风险任务处理规范](./high-risk-task-guideline.md)
- 返回：[Documentation Index](./index.md)

Owner: baoyx · 版本：v1.0 · 生效日期：2026-05-21 · 最后更新：2026-05-21