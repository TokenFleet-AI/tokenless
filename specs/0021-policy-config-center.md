# 0021 — Policy Config Center

> 策略配置中心：按作用域与工具类型管理压缩策略，支持企业级治理、审计与回滚。Spec 完成，待实施。

---

## 1. 背景

当前压缩策略主要由代码中的默认逻辑和 `format_router` 的 `select_strategy` 决定。这种方式在单一产品阶段足够简单，但在以下场景中已经暴露出限制：

- 不同团队对压缩率与保真度的偏好不同。
- 不同工具类型（如 `Bash`、`Read`、`Write`）对响应完整性的要求不同。
- 企业环境需要统一下发强制策略，避免项目自行关闭关键能力。
- 调试、压测、问题回放等临时场景需要会话级覆盖，而不应修改全局默认值。
- 当压缩效果异常时，需要保留变更记录并支持快速回滚。

因此，需要引入一个显式的“策略配置中心”，将策略从硬编码逻辑中抽离为可管理、可覆盖、可审计的配置系统。

目标如下：

1. 支持按工具类型定义压缩策略。
2. 支持 `global` / `project` / `session` 多级作用域。
3. 明确优先级：`session > project > global > default`。
4. 为 CLI、配置文件、审计与企业治理提供统一入口。
5. 在不破坏现有默认行为的前提下平滑演进。

---

## 2. 策略模型

### 2.1 核心概念

策略中心由三层对象组成：

| 对象 | 说明 |
|------|------|
| `CompressionPolicySet` | 某一作用域下的完整策略集合 |
| `ToolPolicy` | 针对某个工具类型的局部覆盖配置 |
| `ResolvedPolicy` | 多级合并后实际生效的最终策略 |

其中，默认策略负责提供全局基线，工具级策略负责按工具类型做最小覆盖。

### 2.2 策略定义

每条策略的核心是“per-tool-type compression profile”，即针对特定工具类型定义压缩行为。基础字段如下：

| 字段 | 类型 | 说明 |
|------|------|------|
| `schema_compression` | enum | Schema 压缩模式 |
| `response_compression` | enum | 响应压缩模式 |
| `rewrite_enabled` | bool | 是否允许重写/改写以提升压缩效果 |
| `cache_enabled` | bool | 是否启用策略相关缓存 |

建议枚举值：

- `schema_compression`: `auto` | `enhanced` | `basic` | `off`
- `response_compression`: `auto` | `aggressive` | `standard` | `high-fidelity` | `off`

说明：

- `auto` 表示交由路由器结合内容结构与上下文自动选择。
- `enhanced` / `aggressive` 偏向更高压缩率。
- `basic` / `standard` 偏向保守压缩。
- `high-fidelity` 优先保真，适合 `Bash`、诊断输出、错误日志等。
- `off` 表示显式关闭该维度压缩。

### 2.3 策略作用域

系统支持三种显式作用域：

| 作用域 | 生命周期 | 典型场景 |
|--------|----------|----------|
| `global` | 跨项目长期生效 | 用户默认偏好、企业基线 |
| `project` | 当前仓库/工作区生效 | 团队标准、仓库特定优化 |
| `session` | 当前 CLI 会话生效 | 调试、实验、临时排障 |

补充说明：

- `global` 适合存储在用户级配置目录中。
- `project` 适合存储在项目本地配置文件中，可进入版本控制或由团队约定是否忽略。
- `session` 不要求持久化到仓库，可由会话状态、环境变量或临时文件承载。

### 2.4 策略优先级

最终策略由多级作用域合并得到，优先级固定为：

```text
session > project > global > default
```

合并规则：

1. 先加载内建 `default` 策略。
2. 叠加 `global` 策略中的同名字段。
3. 再叠加 `project` 策略。
4. 最后叠加 `session` 策略。
5. 若存在工具级配置，工具级配置在其所属作用域内覆盖默认块。
6. 缺失字段不清空下层值，只覆盖显式声明字段。

示例：

- 全局设置 `response_compression = "standard"`
- 项目设置 `Bash.response_compression = "high-fidelity"`
- 会话设置 `cache_enabled = false`

则当前会话中 `Bash` 的最终策略为：

- `response_compression = "high-fidelity"`
- `cache_enabled = false`
- 其余字段沿用项目/全局/默认链路解析结果。

---

## 3. 配置文件格式

策略中心采用 TOML 作为面向用户的主配置格式，兼顾可读性与 CLI 生态兼容性。

基础示例：

```toml
[policies.default]
schema_compression = "auto"   # auto | enhanced | basic | off
response_compression = "auto" # auto | aggressive | standard | high-fidelity | off
rewrite_enabled = true
cache_enabled = true

[policies.tools.Bash]
response_compression = "high-fidelity"

[policies.tools.Read]
response_compression = "basic"
```

### 3.1 结构说明

建议逻辑模型如下：

```toml
[policies.default]
...

[policies.tools.<ToolType>]
...
```

当用于多作用域配置时，可在不同文件承载相同结构，而不是在同一文件中嵌套所有作用域。这样可以减少合并复杂度，并让用户更容易理解“文件所在位置即作用域”。

建议存储位置：

| 作用域 | 建议路径 |
|--------|----------|
| `global` | `~/.tokenfleet-ai/tokenless/policies.toml` |
| `project` | `<repo>/.tokenless/policies.toml` |
| `session` | 运行时状态文件或会话内存，不要求固定路径 |

### 3.2 字段校验

配置加载时必须执行严格校验：

- 工具类型名必须匹配已支持的工具枚举或注册表项。
- 枚举值必须属于允许集合。
- 布尔字段不得接受字符串形式的“宽松解析”。
- 未知顶层字段默认报错，避免静默拼写错误。
- 可提供 `--allow-unknown-tools` 仅用于迁移或实验模式，但默认关闭。

### 3.3 向后兼容

初期可允许旧版默认行为继续生效：

- 若没有任何策略文件，则完全沿用当前代码中的默认选择逻辑。
- 若存在策略文件，则在 `select_strategy` 之前先进行策略解析。
- 代码中的硬编码逻辑退化为 `default` 层的实现来源，而非最终唯一来源。

---

## 4. CLI 命令

策略中心通过 `tokenless policy` 子命令族暴露管理能力。

### 4.1 命令列表

```bash
tokenless policy set
tokenless policy show
tokenless policy list
tokenless policy import
tokenless policy export
```

### 4.2 命令语义

#### `tokenless policy set`

设置某个作用域或工具的策略字段。

示例：

```bash
tokenless policy set --scope global --field response_compression --value standard
tokenless policy set --scope project --tool Bash --field response_compression --value high-fidelity
tokenless policy set --scope session --field cache_enabled --value false
```

行为要求：

- 若目标文件不存在，则自动创建。
- 只更新目标字段，不覆盖无关配置。
- 对非法字段、非法枚举值、未知工具立即报错。
- `session` 作用域默认仅对当前会话有效。

#### `tokenless policy show`

显示某一工具或某一作用域下的策略内容。

示例：

```bash
tokenless policy show --scope project
tokenless policy show --tool Bash
tokenless policy show --tool Read --resolved
```

建议输出模式：

- 默认显示原始配置。
- `--resolved` 显示多级合并后的最终生效结果。
- `--json` / `--toml` 提供机器可读输出。

#### `tokenless policy list`

列出所有已定义策略、来源作用域与覆盖关系。

建议展示：

- 当前已发现的策略文件路径。
- 每个工具的最终策略摘要。
- 哪些字段来自 `session` / `project` / `global` / `default`。

#### `tokenless policy import`

从外部 TOML/JSON 文件导入策略集合。

示例：

```bash
tokenless policy import --scope project ./team-policy.toml
tokenless policy import --scope global ./org-baseline.json
```

导入策略：

- 默认执行校验。
- 支持 `--merge` 与 `--replace` 两种模式。
- `--replace` 属于高影响操作，应要求确认或 `--yes`。

#### `tokenless policy export`

将策略导出为可备份、可分享的文件。

示例：

```bash
tokenless policy export --scope project --output ./project-policy.toml
tokenless policy export --resolved --format json --output ./effective-policy.json
```

---

## 5. 版本管理

企业和团队场景需要对策略变更进行记录、比较与回滚。策略中心应内建轻量版本管理能力。

### 5.1 变更记录

每次通过 CLI 或管理接口写入策略时，记录一条变更事件：

| 字段 | 说明 |
|------|------|
| `version_id` | 版本标识 |
| `timestamp` | 变更时间 |
| `scope` | 变更作用域 |
| `actor` | 变更执行者（CLI 用户/系统） |
| `summary` | 变更摘要 |
| `diff` | 字段级差异 |

建议实现形式：

- 每个作用域维护独立历史。
- 以 append-only 日志存储，避免覆盖历史。
- 差异格式优先使用结构化 JSON diff，而非纯文本 diff。

### 5.2 回滚

支持按版本回滚：

```bash
tokenless policy rollback --scope project --to v12
```

回滚规则：

- 回滚本质上是创建一条新的“恢复到旧内容”的变更，而不是删除中间历史。
- 回滚后应立即重新计算生效策略缓存。
- 对被企业强制锁定的字段，不允许通过回滚绕过治理约束。

### 5.3 对 Git 的关系

项目级策略文件可进入版本控制，但策略中心的运行时历史日志不应默认进入 Git。

建议：

- 配置文件本身可由团队选择纳入仓库。
- 审计日志、快照缓存、回滚索引默认写入用户态或 `.tokenless/state/` 之类的运行时目录。

---

## 6. 企业治理

企业场景下，策略中心不仅是配置系统，也承担治理职责。

### 6.1 强制策略

企业可定义“强制策略”层，逻辑上高于普通用户可编辑作用域。

示例约束：

- 禁止将 `schema_compression` 设为 `off`
- 对 `Bash` 强制 `response_compression = "high-fidelity"`
- 强制开启 `cache_enabled`

建议优先级扩展为：

```text
enforced > session > project > global > default
```

但对外文档中仍可把 `enforced` 作为治理层说明，而不一定暴露为普通用户作用域。

### 6.2 审计日志

对以下操作记录审计事件：

- 策略创建
- 策略修改
- 策略导入
- 策略导出
- 策略回滚
- 强制策略冲突或拒绝

审计字段建议包括：

| 字段 | 说明 |
|------|------|
| `event_id` | 事件 ID |
| `timestamp` | 时间 |
| `actor` | 操作者 |
| `action` | 操作类型 |
| `scope` | 影响作用域 |
| `target` | 默认策略或某个工具 |
| `result` | 成功/拒绝/失败 |
| `reason` | 拒绝原因或补充说明 |

### 6.3 合规与可见性

企业治理模式下，CLI 应明确告知用户：

- 当前是否存在强制策略。
- 某个字段是否因治理被锁定。
- `show --resolved` 时哪些字段来自治理层。
- 用户的 `set` 操作是否被降级、拒绝或部分应用。

---

## 7. 实现路线图

### Phase 1 — 基础配置解耦

目标：从 `format_router` 中抽离硬编码选择逻辑，建立最小可用策略解析层。

范围：

- 定义策略数据模型与枚举。
- 支持 `default` + 工具级覆盖。
- 支持 `global` / `project` 文件加载。
- 在 `select_strategy` 前引入 `ResolvedPolicy` 解析。
- 保持无配置时与当前行为一致。

交付结果：

- 策略文件可被读取。
- `show` / `list` 基础可用。
- 核心压缩路径使用解析后的策略。

### Phase 2 — CLI 管理能力

目标：让用户可通过 CLI 完成常见策略运维。

范围：

- 实现 `set` / `show` / `list` / `import` / `export`。
- 增加配置校验与错误提示。
- 增加 `--resolved` 输出。
- 为会话级覆盖提供临时存储机制。

交付结果：

- 用户无需手改 TOML 即可日常管理策略。
- 能明确查看最终生效策略与来源层级。

### Phase 3 — 版本与回滚

目标：提高变更可追踪性与运维安全性。

范围：

- 变更历史记录。
- 版本快照。
- `rollback` 支持。
- 导入替换前自动备份。

交付结果：

- 策略变更可审计、可恢复。
- 误操作可快速撤销。

### Phase 4 — 企业治理

目标：支持组织级统一控制。

范围：

- 强制策略层。
- 审计日志。
- 锁定字段与冲突提示。
- 未来可扩展到远程下发或集中管理控制面。

交付结果：

- 企业可以统一约束压缩策略。
- 本地 CLI 与治理规则一致协作。

---

## 8. 参考命令与用户流

### 8.1 团队默认策略

```bash
tokenless policy set --scope project --field schema_compression --value enhanced
tokenless policy set --scope project --tool Bash --field response_compression --value high-fidelity
tokenless policy list
```

结果：

- 仓库建立统一项目基线。
- `Bash` 输出优先保真，其余工具按项目默认值处理。

### 8.2 临时调试会话

```bash
tokenless policy set --scope session --field response_compression --value off
tokenless policy show --resolved
```

结果：

- 当前会话关闭响应压缩，用于问题定位。
- 会话结束后不影响项目与全局设置。

### 8.3 导入企业基线

```bash
tokenless policy import --scope global ./enterprise-baseline.toml --merge
tokenless policy export --resolved --format json --output ./effective-policy.json
```

结果：

- 用户级配置吸收企业基线。
- 可导出最终生效策略供审计或排障。

---

## 9. 与现有模块的关系

建议影响模块如下：

| 模块 | 影响 |
|------|------|
| `crates/tokenless-schema/src/format_router.rs` | 将策略选择从硬编码逻辑切换为“解析后策略 + 默认回退” |
| `crates/tokenless-cli/src/main.rs` | 注册 `policy` 子命令 |
| `crates/tokenless-cli/src/...` | 新增 policy 子命令实现、导入导出、显示逻辑 |
| 配置加载模块 | 增加策略文件解析、合并与校验 |
| 会话状态模块 | 支持 `session` 级临时覆盖 |

设计原则：

- 业务代码只依赖 `ResolvedPolicy`，不直接感知多层配置来源。
- CLI 负责编辑与展示，核心库负责解析与合并。
- 默认行为必须可在无配置场景下保持兼容。

---

## 10. 成功标准

当以下条件满足时，可视为策略配置中心达到首个可交付版本：

1. 用户可以通过配置文件或 CLI 为默认策略及工具策略赋值。
2. 系统可按 `session > project > global > default` 正确解析最终策略。
3. `format_router` 在实际压缩流程中使用该最终策略。
4. CLI 能清晰展示原始配置与最终解析结果。
5. 策略变更具备基础历史记录与可回滚能力。
6. 企业模式下可对关键字段施加强制策略并产生日志。

---

## 11. 相关文档

- [0012 — Format Router](./0012-format-router.md)
- [0006 — Error Handling Strategy](./0006-error-handling-strategy.md)
- [0005 — Security Model](./0005-security-model-design.md)
- [0008 — Deployment Architecture](./0008-deployment-architecture.md)
