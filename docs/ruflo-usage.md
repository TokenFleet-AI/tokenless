# Ruflo Usage

English guidance is kept first. A Chinese version is provided after the English section.

This template uses **Ruflo** for agent workflow and orchestration instead of project-local `.claude/skills`.

Ruflo was formerly Claude Flow. In this repository, keep reusable project guidance in tracked Markdown files and keep Ruflo runtime state out of git.

## What Ruflo is used for

Use Ruflo to coordinate larger agent workflows such as:

- requirement analysis and spec generation;
- research / prior-art review;
- implementation planning;
- multi-agent coding, test, review, and documentation passes;
- memory-backed follow-up work across sessions.

For small edits, use the normal agent workflow directly; do not spawn orchestration unless it adds value.

## Repository policy

Tracked project guidance belongs in:

- `CLAUDE.md`;
- `./specs` for requirements, designs, implementation plans, and reviews;
- `./docs` for reusable project documentation.

Do **not** maintain project-local skill trees unless explicitly requested:

- `.claude/skills`.

Ruflo / Claude-flow runtime state is ignored by `.gitignore`, including:

- `.claude-flow/`;
- `.swarm/`;
- `.claude/`;
- `.mcp.json`;
- `ruvector.db`.

## Runtime memory policy

Do not commit Ruflo / Claude-flow runtime memory, sessions, vector databases, logs, or local MCP config. If a memory is useful across machines, summarize and sanitize it into `CLAUDE.md`, `./docs`, or `./specs`.

Use Ruflo only to find candidate memories; keep raw exports outside the repository:

```bash
npx ruflo@latest memory stats
npx ruflo@latest memory search --query "rust template workflow" --limit 10
npx ruflo@latest memory export --namespace "patterns" --output /tmp/ruflo-patterns.json
```

Then ask Claude to summarize without copying raw entries:

```text
Summarize <memory-export-file> for durable project knowledge. Example: summarize `/tmp/ruflo-patterns.json`. Do not repeat raw entries. Redact usernames, absolute paths, tokens, cookies, internal URLs, machine names, and temporary task details. Keep only long-lived project rules, workflows, lessons learned, and decisions. Group the result by target file:
- `CLAUDE.md` for binding project rules;
- `docs/ruflo-usage.md` for Ruflo workflow guidance;
- `docs/pre-commit-usage.md` for pre-commit/tooling notes;
- `docs/troubleshooting.md` for reusable problem/solution notes;
- `docs/research/<topic>.md` for dependency or prior-art research;
- `specs/<feature>-*.md` for feature-specific requirements, designs, and implementation plans.
If a target file does not exist, mark it as "suggested new file". Return concise Markdown patches only.
```

After review, delete the temporary export file.

## Install / init options

Ruflo has two common setup modes.

### Option A: Claude Code plugin mode

Use this when you only want Claude Code slash commands and plugin-provided agents without creating much workspace state.

```text
/plugin marketplace add ruvnet/ruflo
/plugin install ruflo-core@ruflo
/plugin install ruflo-swarm@ruflo
/plugin install ruflo-workflows@ruflo
```

This mode is lighter, but does not register the full Ruflo MCP server.

### Option B: CLI / full loop mode

Use this when you want the full Ruflo loop: MCP server, hooks, daemon, memory, and orchestration.

```bash
npx ruflo@latest init wizard
```

For a non-interactive setup:

```bash
npx ruflo@latest init
```

To register Ruflo as a Claude Code MCP server:

```bash
claude mcp add ruflo -- npx ruflo@latest mcp start
```

Do not commit generated runtime files after running init.

## Daily workflow in this template

1. Keep the user-visible requirement in chat or in `./specs`.
2. Ask Ruflo / the agent to produce or update the relevant spec, research memo, or implementation plan.
3. Store durable decisions in `./specs`, `./docs`, or `CLAUDE.md`.
4. Implement through normal Rust workflow and Makefile targets.
5. Run the smallest validation that proves the change.
6. Keep runtime memories, sessions, daemon state, and local tool config out of git.

## 15-agent reference swarm

The local Ruflo runtime can be configured with `maxAgents: 15`. Treat 15 as an upper bound for complex work, not a default for every task. For small changes, use 1-3 agents or the normal single-agent workflow.

Check available agent types first because Ruflo versions may rename or add types:

```bash
npx ruflo@latest agent list
```

Create a 15-agent swarm for a large task:

```bash
npx ruflo@latest swarm init --topology hierarchical-mesh --max-agents 15 --strategy specialized
npx ruflo@latest agent spawn -t coordinator --name master-coordinator
npx ruflo@latest agent spawn -t researcher --name prior-art-researcher
npx ruflo@latest agent spawn -t architect --name rust-architect
npx ruflo@latest agent spawn -t core-architect --name system-architect
npx ruflo@latest agent spawn -t coder --name core-coder
npx ruflo@latest agent spawn -t coder --name app-coder
npx ruflo@latest agent spawn -t tester --name test-engineer
npx ruflo@latest agent spawn -t test-architect --name test-arch
npx ruflo@latest agent spawn -t reviewer --name code-reviewer
npx ruflo@latest agent spawn -t security-architect --name security-reviewer
npx ruflo@latest agent spawn -t security-auditor --name security-auditor
npx ruflo@latest agent spawn -t memory-specialist --name memory-curator
npx ruflo@latest agent spawn -t performance-engineer --name performance-reviewer
npx ruflo@latest agent spawn -t optimizer --name code-optimizer
npx ruflo@latest agent spawn -t analyst --name requirement-analyst
npx ruflo@latest agent spawn -t swarm-specialist --name swarm-coordinator
```

If one type is unavailable, replace it with the closest type from `agent list`.

| Agent | Role | Best used when |
| --- | --- | --- |
| `master-coordinator` | Keeps scope, order, and exit criteria aligned. | Any multi-agent workflow. |
| `prior-art-researcher` | Compares crates, repos, and current practices. | Dependency or architecture decisions. |
| `rust-architect` | Designs APIs, modules, invariants, and boundaries. | Public API or cross-crate changes. |
| `system-architect` | Deeper system-level architecture and cross-cutting concerns. | Multi-subsystem or performance-critical designs. |
| `core-coder` | Implements library/core crate changes. | Domain logic and reusable APIs. |
| `app-coder` | Implements app/server or integration changes. | Binary, CLI, server, or adapter work. |
| `test-engineer` | Adds unit, integration, and regression tests. | Any production code change. |
| `test-arch` | Designs test strategy, harnesses, and test infrastructure. | Complex test setups or testing architecture. |
| `code-reviewer` | Reviews correctness, maintainability, and drift. | Before declaring implementation done. |
| `security-reviewer` | Checks trust boundaries, secrets, and unsafe patterns. | External input, auth, files, network, deps. |
| `security-auditor` | Deep security audit: OWASP, supply-chain, dependency risk. | Pre-release or high-risk changes. |
| `memory-curator` | Stores reusable decisions and lessons. | Multi-session work or repeated patterns. |
| `performance-reviewer` | Checks unnecessary allocations and hot paths. | Performance-sensitive or async code. |
| `code-optimizer` | Applies targeted optimizations on identified hot paths. | Profile-confirmed bottlenecks. |
| `requirement-analyst` | Turns rough asks into structured requirements and milestones. | New features or unclear scope. |
| `swarm-coordinator` | Manages swarm topology, agent lifecycle, and coordination patterns. | Complex multi-agent orchestration. |

Example prompt for this template:

```text
Use the 15-agent Ruflo swarm for a template maintenance review. Goal: verify CLAUDE.md, docs, Makefile, pre-commit, and cargo-generate settings are consistent. Produce a concise findings list, required file edits, and the smallest validation plan. Do not commit or push.
```

## How to trigger agents from Claude

In Claude, be explicit. Name Ruflo, the swarm size, the roles, the goal, the expected artifacts, and the constraints. A good trigger prompt has this shape:

```text
Use Ruflo <swarm/workflow> with <agent roles>. Goal: <outcome>. Produce: <artifacts>. Constraints: <rules>. Validation: <checks>.
```

Use the full 15-agent swarm only for broad work:

```text
Use Ruflo 15-agent swarm for this repository maintenance task. Spawn coordinator, researcher, architect, core-architect, coders, tester, test-architect, reviewer, security, memory, performance, optimizer, analyst, and swarm-specialist. Goal: review the template workflow end to end and update only docs/config needed for consistency. Produce a findings list, an edit plan, applied changes, and validation results. Follow CLAUDE.md. Do not commit, push, release, or remove template placeholders.
```

Use smaller role sets for focused tasks:

```text
Use Ruflo researcher + architect agents to compare Rust crates for <topic>. Produce docs/research/<topic>-survey.md and list the recommended crate with trade-offs.
```

```text
Use Ruflo tester + reviewer agents to review this change. Check tests, docs, error handling, and security against CLAUDE.md. Produce only actionable findings and the smallest validation command list.
```

Avoid vague triggers such as “optimize this project”. Instead, name the agent roles and the exact deliverable.

## Suggested prompts

Use concise prompts that name the expected artifact:

```text
Use Ruflo to plan this feature and produce specs/<feature>-design.md plus an implementation plan.
```

```text
使用 Ruflo 规划这个功能，并产出 specs/<feature>-design.md 和对应的实现计划。
```

```text
Use Ruflo research workflow to compare current crates for <topic>; put findings under docs/research.
```

```text
使用 Ruflo research workflow 对比当前可用于 <topic> 的 crates，并将调研结论放到 docs/research。
```

```text
Use Ruflo orchestration for implementation review: check tests, docs, security, and error handling against CLAUDE.md.
```

```text
使用 Ruflo orchestration 做实现评审：根据 CLAUDE.md 检查测试、文档、安全性和错误处理。
```

## Template-specific notes

- Preserve template placeholders such as `tokenless`.
- Do not instantiate template variables while maintaining this repository.
- `cargo-generate.toml` already excludes `.claude/*` from generated projects.
- `make check-agent-sync` checks that `CLAUDE.md` exists.

## References

- Ruflo repository: <https://github.com/ruvnet/ruflo>
- Ruflo user guide: <https://github.com/ruvnet/ruflo/blob/main/docs/USERGUIDE.md>

---

## 中文版本

本模板仓库使用 **Ruflo** 作为 agent 工作流和编排工具，不再维护项目内置的 `.claude/skills`。

Ruflo 之前叫 Claude Flow。本仓库的原则是：可复用的项目规范进入 git，Ruflo / Claude-flow 的运行态文件不进入 git。

## Ruflo 用来做什么

Ruflo 适合用来协调较复杂的 agent 工作流，例如：

- 需求分析和规格文档生成；
- 技术调研、竞品 / prior-art 研究；
- 实现计划拆分；
- 多 agent 协作编码、测试、评审和文档检查；
- 基于记忆的跨会话后续工作。

小改动直接使用普通 agent 工作流即可；只有在编排能带来明确收益时才使用 Ruflo。

## 仓库策略

需要进入 git 的长期项目指导应放在：

- `CLAUDE.md`；
- `./specs`：需求、设计、实现计划、评审文档；
- `./docs`：可复用项目文档。

除非明确要求，不再维护项目本地 skill 目录：

- `.claude/skills`。

以下 Ruflo / Claude-flow 运行态已经通过 `.gitignore` 忽略：

- `.claude-flow/`；
- `.swarm/`；
- `.claude/`；
- `.mcp.json`；
- `ruvector.db`。

## 运行态记忆策略

不要提交 Ruflo / Claude-flow 的运行态记忆、session、vector DB、日志或本地 MCP 配置。如果某条记忆需要跨机器复用，应先总结、脱敏，再写入 `CLAUDE.md`、`./docs` 或 `./specs`。

Ruflo 只用于查找候选记忆；原始导出文件应放在仓库外：

```bash
npx ruflo@latest memory stats
npx ruflo@latest memory search --query "rust template workflow" --limit 10
npx ruflo@latest memory export --namespace "patterns" --output /tmp/ruflo-patterns.json
```

然后让 Claude 总结，不要复制原始条目：

```text
请根据 <memory-export-file> 做长期知识归档。例如：请根据 `/tmp/ruflo-patterns.json` 做长期知识归档。不要逐条复述原始 memory。请脱敏用户名、绝对路径、token、cookie、内部 URL、机器名和临时任务细节。只保留长期有效的项目规则、工作流、踩坑经验和决策。请按目标文件分类输出：
- `CLAUDE.md`：强约束项目规则；
- `docs/ruflo-usage.md`：Ruflo 工作流说明；
- `docs/pre-commit-usage.md`：pre-commit / 工具链说明；
- `docs/troubleshooting.md`：可复用的问题和解决方案；
- `docs/research/<topic>.md`：依赖选择或 prior-art 调研；
- `specs/<feature>-*.md`：具体功能的需求、设计和实现计划。
如果目标文件不存在，请标记为“建议新建文件”。最后只输出精简 Markdown patch 建议。
```

人工检查总结内容后，删除临时导出文件。

## 安装 / 初始化方式

Ruflo 常见有两种使用模式。

### 方式 A：Claude Code 插件模式

适合只需要 Claude Code slash commands 和插件提供的 agents，不想在工作区生成太多运行态文件的场景。

```text
/plugin marketplace add ruvnet/ruflo
/plugin install ruflo-core@ruflo
/plugin install ruflo-swarm@ruflo
/plugin install ruflo-workflows@ruflo
```

这种方式较轻量，但不会注册完整的 Ruflo MCP server。

### 方式 B：CLI / 完整工作流模式

适合需要完整 Ruflo loop 的场景：MCP server、hooks、daemon、memory、orchestration 等。

交互式初始化：

```bash
npx ruflo@latest init wizard
```

非交互式初始化：

```bash
npx ruflo@latest init
```

将 Ruflo 注册为 Claude Code MCP server：

```bash
claude mcp add ruflo -- npx ruflo@latest mcp start
```

初始化后不要提交生成的运行态文件。

## 本模板的日常工作流

1. 将用户可见需求保留在对话中，或整理到 `./specs`。
2. 让 Ruflo / agent 生成或更新对应的 spec、research memo、implementation plan。
3. 将长期有效的决策写入 `./specs`、`./docs` 或 `CLAUDE.md`。
4. 实现阶段使用正常 Rust 工作流和 Makefile target。
5. 运行能证明本次改动正确性的最小验证。
6. 不提交 memories、sessions、daemon state、本地工具配置等运行态内容。

## 15 个智能体参考编排

本地 Ruflo runtime 可以配置 `maxAgents: 15`。这里的 15 是复杂任务的上限，不是每次都必须创建 15 个智能体。小改动建议使用 1-3 个智能体，或者直接使用普通单 agent 工作流。

先查看当前 Ruflo 版本支持哪些 agent 类型：

```bash
npx ruflo@latest agent list
```

为大型任务创建 15 个智能体：

```bash
npx ruflo@latest swarm init --topology hierarchical-mesh --max-agents 15 --strategy specialized
npx ruflo@latest agent spawn -t coordinator --name master-coordinator
npx ruflo@latest agent spawn -t researcher --name prior-art-researcher
npx ruflo@latest agent spawn -t architect --name rust-architect
npx ruflo@latest agent spawn -t core-architect --name system-architect
npx ruflo@latest agent spawn -t coder --name core-coder
npx ruflo@latest agent spawn -t coder --name app-coder
npx ruflo@latest agent spawn -t tester --name test-engineer
npx ruflo@latest agent spawn -t test-architect --name test-arch
npx ruflo@latest agent spawn -t reviewer --name code-reviewer
npx ruflo@latest agent spawn -t security-architect --name security-reviewer
npx ruflo@latest agent spawn -t security-auditor --name security-auditor
npx ruflo@latest agent spawn -t memory-specialist --name memory-curator
npx ruflo@latest agent spawn -t performance-engineer --name performance-reviewer
npx ruflo@latest agent spawn -t optimizer --name code-optimizer
npx ruflo@latest agent spawn -t analyst --name requirement-analyst
npx ruflo@latest agent spawn -t swarm-specialist --name swarm-coordinator
```

如果某个类型不存在，用 `agent list` 里最接近的类型替换。

| 智能体 | 作用 | 适用场景 |
| --- | --- | --- |
| `master-coordinator` | 控制范围、顺序和完成标准。 | 任何多智能体任务。 |
| `prior-art-researcher` | 对比 crates、仓库和当前实践。 | 依赖或架构决策前。 |
| `rust-architect` | 设计 API、模块、边界和不变量。 | 公共 API 或跨 crate 改动。 |
| `system-architect` | 更深层系统级架构和跨关注点设计。 | 多子系统或性能敏感的架构设计。 |
| `core-coder` | 实现 library/core crate 改动。 | 领域逻辑和可复用 API。 |
| `app-coder` | 实现 app/server 或集成改动。 | binary、CLI、server、adapter。 |
| `test-engineer` | 补充单元、集成和回归测试。 | 任何生产代码改动。 |
| `test-arch` | 设计测试策略、测试框架和测试基础设施。 | 复杂测试架构或测试体系设计。 |
| `code-reviewer` | 检查正确性、可维护性和实现漂移。 | 宣称完成前。 |
| `security-reviewer` | 检查信任边界、secret 和不安全模式。 | 外部输入、认证、文件、网络、依赖。 |
| `security-auditor` | 深度安全审计：OWASP、供应链、依赖风险。 | 发布前或高风险改动。 |
| `memory-curator` | 记录可复用决策和经验。 | 跨会话或重复模式工作。 |
| `performance-reviewer` | 检查无谓分配和热点路径。 | 性能敏感或 async 代码。 |
| `code-optimizer` | 针对已确认热点进行定向优化。 | 性能分析已确认瓶颈。 |
| `requirement-analyst` | 将粗略需求拆成结构化需求和里程碑。 | 新功能或范围不清晰时。 |
| `swarm-coordinator` | 管理 swarm 拓扑、智能体生命周期和协作模式。 | 复杂多智能体编排。 |

本模板仓库范例提示词：

```text
使用 15 个智能体的 Ruflo swarm 做一次模板维护评审。目标：确认 CLAUDE.md、docs、Makefile、pre-commit、cargo-generate 配置保持一致。产出简洁的问题列表、需要修改的文件和最小验证计划。不要 commit 或 push。
```

## 在 Claude 中如何触发这些智能体

在 Claude 对话里要明确描述：使用 Ruflo、智能体数量、角色、目标、产出物和约束。推荐格式：

```text
使用 Ruflo <swarm/workflow>，启用 <智能体角色>。目标：<结果>。产出：<文件或报告>。约束：<规则>。验证：<检查命令>。
```

只有在范围较大的任务里才使用完整 15 个智能体：

```text
使用 Ruflo 15-agent swarm 处理这个仓库维护任务。启用 coordinator、researcher、architect、core-architect、coders、tester、test-architect、reviewer、security、memory、performance、optimizer、analyst、swarm-specialist 这些角色。目标：端到端审查模板工作流，并且只更新保持一致性所需的 docs/config。产出问题列表、修改计划、实际变更和验证结果。遵守 CLAUDE.md。不要 commit、push、release，也不要替换模板占位符。
```

聚焦任务用少量智能体即可：

```text
使用 Ruflo researcher + architect 智能体对比用于 <topic> 的 Rust crates。产出 docs/research/<topic>-survey.md，并列出推荐 crate 和取舍原因。
```

```text
使用 Ruflo tester + reviewer 智能体评审这次改动。根据 CLAUDE.md 检查测试、文档、错误处理和安全性。只产出可执行问题和最小验证命令列表。
```

避免使用“帮我优化这个项目”这类模糊描述；应明确智能体角色和具体产出物。

## 推荐提示词

提示词应简洁，并明确产出物：

```text
Use Ruflo to plan this feature and produce specs/<feature>-design.md plus an implementation plan.
```

```text
使用 Ruflo 规划这个功能，并产出 specs/<feature>-design.md 和对应的实现计划。
```

```text
Use Ruflo research workflow to compare current crates for <topic>; put findings under docs/research.
```

```text
使用 Ruflo research workflow 对比当前可用于 <topic> 的 crates，并将调研结论放到 docs/research。
```

```text
Use Ruflo orchestration for implementation review: check tests, docs, security, and error handling against CLAUDE.md.
```

```text
使用 Ruflo orchestration 做实现评审：根据 CLAUDE.md 检查测试、文档、安全性和错误处理。
```

## 模板仓库注意事项

- 保留 `tokenless` 等模板占位符。
- 维护模板仓库时，不要把模板变量实例化成具体项目名。
- `cargo-generate.toml` 已经排除 `.claude/*`，生成的新项目不会带出这些运行态目录。
- `make check-agent-sync` 检查 `CLAUDE.md` 存在。

## 参考资料

- Ruflo 仓库：<https://github.com/ruvnet/ruflo>
- Ruflo 用户指南：<https://github.com/ruvnet/ruflo/blob/main/docs/USERGUIDE.md>
