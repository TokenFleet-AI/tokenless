# CodeGraph 使用教程

本教程说明“代码图谱 / 代码关系分析”类工具的用途，并基于 `colbymchenry/codegraph` 补充一个可落地的安装、初始化、更新和 Claude Code 接入参考。

Owner: baoyx · 版本：v1.0 · 生效日期：2026-05-21

## 1. 本文对应的官方项目

本文对应的 `CodeGraph` 是这个项目：

- GitHub：<https://github.com/colbymchenry/codegraph>
- npm：<https://www.npmjs.com/package/@colbymchenry/codegraph>
- README：<https://github.com/colbymchenry/codegraph/blob/main/README.md>
- Releases：<https://github.com/colbymchenry/codegraph/releases>

后面的安装、初始化、更新、Claude Code 接入步骤，都以这个仓库的 README 为准。

## 2. CodeGraph 是什么

CodeGraph 这类工具的核心价值，是把代码中的结构和关系变成可查询、可导航、可分析的图谱能力，例如：

- 文件与模块关系；
- 函数/类型的定义与引用；
- 调用链与依赖方向；
- 某次改动可能影响到的上下游代码。

它适合回答“这段代码和谁有关”“改这里会影响哪里”“某个功能到底落在哪些文件里”这类问题。

## 3. 什么时候用

适合使用 CodeGraph 的场景：

- 首次接手一个不熟悉的仓库；
- 要修改跨模块功能，先看影响面；
- 要定位某个入口最终落到哪里；
- 做重构前，先理解调用链和边界；
- 做评审时，检查依赖方向是否合理。

不适合替代的事情：

- 不能替代测试结果；
- 不能替代真实运行时行为分析；
- 不能替代对业务规则的人工确认。

## 4. 官方链接

下面这些链接是当前可查到的官方入口：

- GitHub 仓库：<https://github.com/colbymchenry/codegraph>
- README：<https://github.com/colbymchenry/codegraph/blob/main/README.md>
- npm 包：<https://www.npmjs.com/package/@colbymchenry/codegraph>
- Releases：<https://github.com/colbymchenry/codegraph/releases>

## 5. 安装方式

### 方式 A：推荐方式，直接运行交互式安装器

官方 README 的最短开始方式是：

```bash
npx @colbymchenry/codegraph
```

这个交互式安装器会自动做几件事：

- 检测可用 agent，例如 Claude Code、Cursor、Codex CLI、OpenCode；
- 提示是否把 `codegraph` 装到 PATH；
- 询问配置是全局还是当前项目；
- 写入对应 agent 的 MCP 配置和说明文件；
- 在选择 Claude Code 时，可顺手配置 auto-allow 权限。

### 方式 B：显式安装 CLI

如果你不想用 `npx` 临时运行，也可以全局安装：

```bash
npm install -g @colbymchenry/codegraph
```

安装后可用这些命令：

```bash
codegraph --help
codegraph install
codegraph init
codegraph serve --mcp
```

### 方式 C：非交互方式

适合脚本化、CI 或你已经知道自己要接哪些 agent：

```bash
codegraph install --yes
codegraph install --target=cursor,claude --yes
codegraph install --target=auto --location=local
codegraph install --print-config codex
```

常见参数：

- `--target`: 指定目标 agent；
- `--location`: `global` 或 `local`；
- `--yes`: 跳过交互；
- `--print-config`: 只打印配置片段，不直接写文件。

## 6. 如何初始化

这个项目的初始化很直接：进入项目目录后执行：

```bash
cd your-project
codegraph init -i
```

这里的 `-i` 表示初始化并建立当前项目的索引。初始化完成后，项目里会出现 `.codegraph/` 目录，后续 Claude Code / Cursor 等 agent 就会开始使用它。

常见相关命令：

```bash
codegraph init [path]
codegraph uninit [path]
codegraph index [path]
codegraph sync [path]
codegraph status [path]
```

典型含义：

- `init -i`: 首次初始化并建索引；
- `index`: 全量重建索引；
- `sync`: 增量更新索引；
- `status`: 查看索引状态、统计信息、后端类型；
- `uninit`: 删除当前项目的 CodeGraph 初始化状态。

## 7. 如何更新

最常见的更新方式是重新安装最新版 npm 包：

```bash
npm install -g @colbymchenry/codegraph@latest
```

更新后建议再做两步：

```bash
codegraph install --target=claude --yes
codegraph status
```

如果当前项目已经初始化过，再做一次索引同步通常更稳：

```bash
codegraph sync
```

如果你怀疑索引损坏、版本升级跨度较大，或者结构变化很多，可以直接全量重建：

```bash
codegraph index --force
```

另外，官方 release 页也可以用来确认最新版本：

- <https://github.com/colbymchenry/codegraph/releases>

## 8. Claude Code 实际接入步骤

下面给出一个偏实操的接入流程，目标是让 Claude Code 在当前机器上通过 MCP 使用 CodeGraph。

### 步骤 1：安装 CodeGraph

最简单的方式：

```bash
npx @colbymchenry/codegraph
```

或者先全局安装：

```bash
npm install -g @colbymchenry/codegraph
```

确认命令可用：

```bash
codegraph --help
```

### 步骤 2：把 CodeGraph 注册到 Claude Code 的 MCP 配置

如果你直接运行了交互式安装器，通常它会自动帮你写好 Claude Code 的 MCP 配置。

如果要手动配置，README 里的 `~/.claude.json` 示例是：

```json
{
  "mcpServers": {
    "codegraph": {
      "type": "stdio",
      "command": "codegraph",
      "args": ["serve", "--mcp"]
    }
  }
}
```

可选地，你还可以按 README 给 Claude Code 增加 auto-allow 权限，在 `~/.claude/settings.json` 中加入：

```json
{
  "permissions": {
    "allow": [
      "mcp__codegraph__codegraph_search",
      "mcp__codegraph__codegraph_context",
      "mcp__codegraph__codegraph_callers",
      "mcp__codegraph__codegraph_callees",
      "mcp__codegraph__codegraph_impact",
      "mcp__codegraph__codegraph_node",
      "mcp__codegraph__codegraph_status",
      "mcp__codegraph__codegraph_files"
    ]
  }
}
```

建议：

- 如果你已经有别的 MCP server，不要覆盖原有配置，只新增 `codegraph` 这一项；
- 修改配置后，重启 Claude Code。

### 步骤 3：重启 Claude Code 并打开仓库根目录

重启 Claude Code 后，打开目标仓库根目录，而不是只打开某个子目录。对这个模板仓库来说，应直接打开仓库根目录。

这样 CodeGraph 更容易正确理解：

- `Cargo.toml` workspace；
- `apps/server` 应用入口；
- `crates/core` 共享库；
- `docs`、`specs`、`Makefile` 等辅助结构。

### 步骤 4：初始化当前仓库

这一步很关键。对当前仓库，在仓库根目录执行：

```bash
cd /path/to/rust-lib-template
codegraph init -i
```

执行后建议检查：

```bash
codegraph status
```

首次进入仓库后，可以再给 Claude 一个明确任务，例如：

- 使用 CodeGraph 总结这个仓库结构；
- 使用 CodeGraph 找出 Rust workspace 的入口点；
- 使用 CodeGraph 分析 `apps/server` 和 `crates/core` 的关系。

### 步骤 5：第一次验证接入是否成功

建议第一次不要直接问复杂问题，而是先做三个简单验证：

1. 能否看到仓库结构；
2. 能否识别入口点；
3. 能否回答某个模块的上下游关系。

可以直接这样问 Claude：

- `Use CodeGraph to summarize this repository structure.`
- `Use CodeGraph to identify the main Rust workspace entry points.`
- `Use CodeGraph to show how apps/server relates to crates/core.`

如果这些问题都能正常回答，并且仓库里已经有 `.codegraph/` 目录，通常就说明 CodeGraph 已经接入成功。

## 9. “首次接入这个 Rust 模板仓库”的示例流程

下面是一套更贴近当前仓库的首次接入流程，适合第一次把 `colbymchenry/codegraph` 接到这个 Rust 模板仓库时使用。

### 0. 先安装并初始化

建议在仓库根目录直接完成：

```bash
npm install -g @colbymchenry/codegraph
cd /path/to/rust-lib-template
codegraph init -i
codegraph status
```

如果你更喜欢交互式安装器，也可以先执行：

```bash
npx @colbymchenry/codegraph
```

然后再回到仓库根目录执行 `codegraph init -i`。

### 第一步：先做仓库总览

先让 CodeGraph 识别整个仓库的骨架，而不是一开始就钻进某个函数：

- `Cargo.toml` 是否是 workspace 根；
- 有哪些 app / crate；
- 哪些目录是代码，哪些目录是文档和规范。

建议提示词：

- `Use CodeGraph to summarize the top-level structure of this Rust template repository.`
- `Identify the main workspace members and explain their roles.`

对这个模板仓库，重点一般会落在：

- `apps/server`
- `crates/core`
- `docs`
- `specs`
- `Makefile`
- `CLAUDE.md`

### 第二步：识别入口点和边界

接下来不要马上改代码，而是先确认入口点和模块边界：

- 二进制入口在哪里；
- 核心库暴露了哪些能力；
- 哪些 crate/模块属于公共边界。

建议提示词：

- `Use CodeGraph to find the main entry points in this repository.`
- `Use CodeGraph to identify public API boundaries in crates/core.`
- `Trace the startup path for apps/server.`

### 第三步：理解依赖方向和影响面

首次接入时，最有价值的不是“看全图”，而是确认依赖方向是否清晰：

- `apps/server` 是否依赖 `crates/core`；
- `crates/core` 是否保持相对稳定的公共边界；
- 修改公共 API 时会影响哪些调用点。

建议提示词：

- `Use CodeGraph to show the dependency relationship between apps/server and crates/core.`
- `Analyze the impact of changing public interfaces in crates/core.`

### 第四步：结合模板仓库规则理解工作流

这个仓库不是普通业务仓库，而是模板仓库，所以首次接入时最好顺便让 CodeGraph 帮助你确认“代码结构”和“仓库规则”是如何配合的。

重点包括：

- `CLAUDE.md` 中的模板与工作流约束；
- `docs/` 下的使用说明；
- `Makefile` 中约定的构建、测试、lint 入口；
- `specs/` 中规范文档的放置方式。

建议提示词：

- `Use CodeGraph to summarize how code, docs, and specs are organized in this template.`
- `Identify the implementation entry points and the validation entry points defined by the Makefile.`

### 第五步：再进入具体任务

完成上面 4 步后，再进入具体修改会更稳。典型顺序是：

1. 用 CodeGraph 定位要改的模块；
2. 用 CodeGraph 看调用链和影响面；
3. 再根据 [SPARC 使用规范](./sparc-usage-guideline.md) 判断任务复杂度；
4. 如果任务较复杂，再按 [Ruflo Usage](./ruflo-usage.md) 组织多 agent 协作。

经验法则：首次接入这个模板仓库时，先让 CodeGraph 帮你建立“结构地图”，再开始做改动。

### 一个最小可执行示例

如果你只想快速走通一次，可以按下面顺序：

```bash
npm install -g @colbymchenry/codegraph
cd /path/to/rust-lib-template
codegraph init -i
codegraph status
```

然后在 Claude Code 里问：

- `Use CodeGraph to summarize the Rust workspace structure in this repository.`
- `Use CodeGraph to find the main entry points and public API boundaries.`
- `Use CodeGraph to analyze how apps/server depends on crates/core.`

如果 Claude 能直接基于这些关系回答，而不是到处扫文件，说明接入已经基本正常。

## 10. 一个实用工作流

### 场景 A：修一个已知 bug

- 从报错点、日志关键词或目标函数开始；
- 顺着调用链定位真正负责逻辑的实现；
- 反查该函数是否被别处复用；
- 再决定是局部修复还是需要补测试。

### 场景 B：做跨模块改动

- 先看模块依赖图；
- 找到跨模块边界：公共 trait、DTO、错误类型、配置结构；
- 列出所有引用点；
- 按“实现 → 测试 → 评审”推进，避免漏改。

### 场景 C：做重构或删除旧代码

- 先确认定义点；
- 再查所有引用点；
- 再看是否存在间接依赖；
- 确认无引用后，再删除或迁移。

## 11. 和 SPARC / Ruflo 的关系

- **CodeGraph**：负责帮助你理解代码关系和影响面；
- **SPARC**：负责告诉你该用什么协作模式处理这个任务；
- **Ruflo**：负责把多 agent 的分工与编排真正跑起来。

可以这样理解：

- CodeGraph 解决“看清代码”；
- SPARC 解决“怎么组织任务”；
- Ruflo 解决“怎么执行编排”。

例如，一个跨模块重构任务可以先用 CodeGraph 判断影响面，再按 [SPARC 使用规范](./sparc-usage-guideline.md) 决定是否属于高风险任务，最后用 [Ruflo Usage](./ruflo-usage.md) 所描述的编排方式推进实现、测试和评审。

## 12. 常见误区

- **误区 1：图上能连起来，就说明真实运行一定会经过。**
  静态关系不等于真实运行路径，尤其在动态分发、配置切换、条件编译场景下更要小心。

- **误区 2：引用少，就说明可以放心删。**
  删除前仍要结合测试、文档、公开 API 兼容性一起判断。

- **误区 3：图越大越有价值。**
  真正常用的是局部视图：围绕当前任务看最小必要上下文。

## 13. 建议产出物

在复杂任务里，使用 CodeGraph 后建议把结论沉淀成简短文字，而不是只停留在图上：

- 入口点；
- 关键调用链；
- 影响模块列表；
- 高风险边界；
- 需要补的测试点。

如果这些信息对后续多人协作仍然有价值，可以整理进 `./specs` 或 `./docs`。

## 导航

- 相关文档：[SPARC 使用规范](./sparc-usage-guideline.md)
- 相关文档：[Ruflo Usage](./ruflo-usage.md)
- 返回：[Documentation Index](./index.md)

Owner: baoyx · 版本：v1.0 · 生效日期：2026-05-21 · 最后更新：2026-05-21