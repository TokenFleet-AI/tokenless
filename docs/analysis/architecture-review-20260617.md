# Tokenless 架构评审（2026-06-17）

## 总体结论

**总体架构评分：7.5 / 10**

理由：
- Workspace 分层已经基本成型，`schema / stats / semantic / cli / tui` 的主职责比早期集中式结构清晰得多。
- 依赖方向总体健康，核心压缩能力位于库层，CLI 主要承担编排与协议适配，符合模板化 Rust workspace 的基本原则。
- `main.rs` 已不再是 spec 0016 中描述的 1300+ 行巨石，CLI 拆分事实上已经完成一大半。
- 但仍存在三类明显架构债务：一是规格文档与代码现状不一致；二是公共 API 暴露面偏宽，部分 crate 仍带有“内部实现顺手 pub 出去”的痕迹；三是扩展点还停留在条件分支和模块拼接层，没有形成正式的策略插件边界。
- 此外，`tokenless-semantic` 已引入潜在异步/远程模型场景，但系统主干仍按同步 CLI 假设设计，未来若接入远程 embedding、模型下载、在线策略学习，会很快触到同步边界瓶颈。

---

## 1. Crate 职责边界

### 结论

**整体清晰，但边界比 spec 中更复杂，且已有新职责溢出到 CLI。**

### 发现

#### 1.1 `tokenless-schema` 仍然是核心压缩引擎

从导出面看，`tokenless-schema` 主要负责：
- Schema 压缩
- Response 压缩
- JSON 结构分析
- 格式路由
- TOON/CJSON 编码

对应入口集中在：
- `/Users/byx/Documents/workspace/github.com/TokenFleet-AI/tokenless/crates/tokenless-schema/src/lib.rs:24`
- `/Users/byx/Documents/workspace/github.com/TokenFleet-AI/tokenless/crates/tokenless-schema/src/lib.rs:39`

这部分边界基本合理：它仍是“纯压缩引擎层”。

#### 1.2 `tokenless-stats` 不只是存储，已经兼具查询格式化与日志侧车职责

`tokenless-stats` 不仅暴露存储与统计模型，还直接暴露：
- 文本格式化函数 `format_summary / format_list / format_diff / format_show`
- `compress_log` 这种偏 hook 运维侧日志能力

见：
- `/Users/byx/Documents/workspace/github.com/TokenFleet-AI/tokenless/crates/tokenless-stats/src/lib.rs:22`
- `/Users/byx/Documents/workspace/github.com/TokenFleet-AI/tokenless/crates/tokenless-stats/src/lib.rs:32`

这说明它已从“数据持久层”扩展成“统计应用服务层”。对 CLI 来说这很方便，但从职责边界看，展示格式和存储查询耦在一起，后续若想给 TUI、MCP、Web UI 提供不同展示层，会受限。

#### 1.3 `tokenless-semantic` 已经成为独立能力域

`tokenless-semantic` 不是 `schema` 的子模块，而是单独 crate，负责：
- 上下文分类
- 语义字段保留/丢弃
- 可选 ONNX embedder
- 模型下载与缓存

见：
- `/Users/byx/Documents/workspace/github.com/TokenFleet-AI/tokenless/crates/tokenless-semantic/src/lib.rs:10`
- `/Users/byx/Documents/workspace/github.com/TokenFleet-AI/tokenless/crates/tokenless-semantic/src/lib.rs:68`

这是合理的，因为它依赖特征、失败模式、部署约束都与结构压缩不同。但问题在于：**spec 0001 的 Layer 3 仍只列出 schema/stats，没有把 semantic 作为正式层级成员**，说明架构文档已经落后于实现。

#### 1.4 `tokenless-cli` 仍然承载了过多“应用编排 + 协议适配 + 状态缓存”职责

CLI 当前负责：
- clap 命令解析
- agent hook 协议适配
- env-check
- init 安装
- MCP server 启动
- cache / diff cache
- TUI 入口编排

其中最重的热点模块是：
- `/Users/byx/Documents/workspace/github.com/TokenFleet-AI/tokenless/crates/tokenless-cli/src/commands/hook.rs`

该文件单体约 **849 行**，远大于其他 handler，说明“协议适配层”仍是 CLI 内部的次级巨石。

#### 1.5 `tokenless-tui` 与 `apps/tui` 的双层结构基本合理

当前：
- `crates/tokenless-tui` 是库
- `apps/tui` 是独立二进制包装器

与 spec 0016 推荐的方案 A 一致，属于健康演进。

### 判断

- 职责主轴是清晰的。
- 但**“统计展示”与“统计存储”仍混在一个 crate**。
- **“hook 协议适配”尚未从 CLI 中抽出成独立应用服务层或协议层 crate**。
- **语义压缩已经形成新子域，但架构文档仍未升级到反映该现实。**

---

## 2. 依赖方向健康度

### 结论

**当前依赖图单向、无明显循环，但已不再是 spec 0016 中的 `core ← schema/stats ← cli` 严格形态。**

### 当前内部依赖

基于 `cargo metadata`：
- `tokenless-schema` → 无内部依赖
- `tokenless-semantic` → 无内部依赖
- `tokenless-stats` → 无内部依赖
- `tokenless-tui` → `tokenless-schema`, `tokenless-stats`
- `tokenless`(CLI) → `tokenless-schema`, `tokenless-semantic`, `tokenless-stats`, `tokenless-tui`
- `tless-tui` → `tokenless-stats`, `tokenless-tui`

### 发现

#### 2.1 没有循环依赖

从 workspace metadata 看，没有 crate 相互回指，整体仍是 DAG，这一点健康。

#### 2.2 spec 0016 中的 `crates/core` 方案已被放弃或回退

spec 0016 目标是：
- `core` 作为共享类型底座
- `schema/stats` 建立在 core 之上
- `cli/tui` 再依赖其上层

但当前 git 状态显示 `crates/core` 已被删除，workspace 中也不存在该包；因此：
- 规范中的理想依赖方向未落地
- 当前系统实际上是 **多核心并列库 + CLI 汇聚** 架构，而不是明确的分层基座架构

这并不一定更差，但意味着：**文档声称的分层，不是代码真实分层。**

#### 2.3 `tokenless-tui` 依赖 `tokenless-schema` 值得复查

按产品职责，TUI 主要看统计数据与配置；若 TUI 需要调用压缩引擎做 demo/预览，还说得通，但作为 dashboard 库依赖 `tokenless-schema` 会让 UI 层接触压缩策略细节，增加耦合。

这不是严重问题，但从依赖纯度看，最好确认它是否真的需要此依赖。如果只是少量 `Strategy` 展示或工具函数，可考虑转移类型定义或在 stats 层提供只读投影。

### 判断

- 依赖图健康度：**8/10**。
- 优点：无环、单向、CLI 汇聚式结构易理解。
- 缺点：**不满足 spec 0016 设想的严格层次化依赖方向**，且缺少真正的共享核心域模型。

---

## 3. 公共 API 设计

### 结论

**公共 API 整体可用，但暴露面偏大，部分 crate 还没有收敛到“最小稳定接口”。**

### 发现

#### 3.1 `tokenless-schema` 的对外 API 相对合理，但仍偏“工具箱式”

导出包括：
- `SchemaCompressor`
- `ResponseCompressor`
- `CompressionProfile`
- `JsonShape`
- `TopType`
- `Strategy`
- `compress_auto / compress_auto_with / select_strategy / strategy_name`
- `analyze`

见：
- `/Users/byx/Documents/workspace/github.com/TokenFleet-AI/tokenless/crates/tokenless-schema/src/lib.rs:39`

这套 API 对库用户很方便，但有两个问题：
1. `analyze` 与 `select_strategy` 直接公开，意味着外部代码可以绑定内部启发式细节；以后调整路由算法时兼容压力变大。
2. `Strategy` 已成为用户可感知契约，但其本质更像内部规划结果，而非必须稳定的业务语义。

更稳妥的设计是：
- 保留高层稳定 API：`compress_schema`, `compress_response`, `compress_auto_with`
- 将 shape/router 级 API 降级为 `pub(crate)` 或 feature-gated advanced API

#### 3.2 `tokenless-stats` 暴露了较多展示层函数

`format_*` 系列由根 lib 直接 re-export：
- `/Users/byx/Documents/workspace/github.com/TokenFleet-AI/tokenless/crates/tokenless-stats/src/lib.rs:32`

这让 crate 既像存储层，又像 CLI 呈现 SDK。短期方便，长期会让：
- CLI 文本格式成为库兼容面
- TUI/Web/MCP 复用时被迫绕开或继承文本模型

#### 3.3 `tokenless-semantic` API 偏原始对象式，缺少能力分层

当前公开接口主要是：
- `SemanticCompressor::new`
- `load_onnx`
- `compress`
- `is_field_kept`
- `detect_category`

见：
- `/Users/byx/Documents/workspace/github.com/TokenFleet-AI/tokenless/crates/tokenless-semantic/src/lib.rs:101`

问题不在数量，而在抽象层次：
- “分类”与“压缩”耦在一个对象上
- ONNX 加载行为混进主对象生命周期
- 未来若增加远程 embedding provider、缓存层、策略版本，会比较难扩展

更好的方向是拆成：
- `SemanticPolicyEngine`
- `ContextClassifier`
- `EmbeddingBackend` trait / enum
- `SemanticCompressionDecision`

#### 3.4 CLI crate 存在不必要的 pub 暴露

例如：
- `/Users/byx/Documents/workspace/github.com/TokenFleet-AI/tokenless/crates/tokenless-cli/src/cache.rs`
- `/Users/byx/Documents/workspace/github.com/TokenFleet-AI/tokenless/crates/tokenless-cli/src/init/mod.rs`
- `/Users/byx/Documents/workspace/github.com/TokenFleet-AI/tokenless/crates/tokenless-cli/src/mcp.rs:101`

这些大多是二进制内部模块，却以 `pub` 导出结构和函数。虽然 Rust binary crate 的“公开”不会自动对外发布成库 API，但这会在内部形成“到处可用”的松散边界，降低模块封装性。

### 判断

公共 API 最主要的问题不是“已经不可控”，而是：
- 缺少明确的稳定层 / 高级层区分
- 内部启发式和展示逻辑泄漏到了根导出面

建议评分：**6.5/10**。

---

## 4. 异步 / 同步边界

### 结论

**当前主干几乎是同步架构，但未来扩展已经明显触碰 async 边界。**

### 发现

#### 4.1 当前系统主路径仍是同步 CLI 心智模型

从 `tokenless-cli/src/main.rs` 看，整体调度仍是同步执行：
- 解析命令
- 同步调用 handler
- 同步读写 stdin/stdout / 文件 / SQLite

没有 Tokio runtime，也没有显式 async handler，这对于本地 CLI 非常合理。

#### 4.2 `tokenless-semantic` 已经引入潜在阻塞型外部操作

在 `tokenless-semantic` 中，Level 2 ONNX 与模型下载已经出现：
- 模型缓存目录管理
- 首次下载模型文件
- ONNX 推理加载
- 可选 `ureq` 远程请求依赖

见：
- `/Users/byx/Documents/workspace/github.com/TokenFleet-AI/tokenless/crates/tokenless-semantic/Cargo.toml:19`
- `/Users/byx/Documents/workspace/github.com/TokenFleet-AI/tokenless/crates/tokenless-semantic/src/lib.rs:110`

这意味着：
- 当前同步模式下，首次启用语义压缩可能阻塞 CLI 较长时间
- 若未来引入远程 embedding API，现有接口会让网络时延直接侵入压缩主路径

#### 4.3 MCP server 仍是同步 stdio 处理

MCP 子命令目前是 CLI 里的同步入口，而不是独立 async server runtime。对 stdio MCP 来说现在够用，但一旦要支持：
- 并发工具调用
- 长时运行的模型预热
- 后台下载 / 索引更新
- streaming / partial results

现有同步实现会变成限制。

### 什么时候需要 async

建议只在以下场景正式引入 async，而不是提前全面异步化：
1. 远程 embedding / 远程语义策略 API
2. MCP server 需要并发处理多个请求
3. 模型下载、缓存更新、统计上报转后台任务
4. TUI 或 daemon 需要异步订阅事件流

### 建议

维持当前“CLI 默认同步”是对的，但应尽快建立**异步边界隔离层**：
- 库层用 trait 抽象 `EmbeddingProvider`
- CLI 层同步调用 local provider
- 未来 remote provider 通过 feature 或 adapter 切到 async

也就是说，**现在不一定要引入 Tokio 到主干，但要先设计 async-ready 的抽象。**

---

## 5. 扩展性瓶颈

### 结论

**当前新增“一个命令 / 一个策略”还算容易，但新增“一个能力子系统”会开始吃力。**

### 主要瓶颈

#### 5.1 压缩策略扩展仍依赖集中式路由分支

`FormatRouter` 现在是固定枚举 + 固定条件判断：
- `/Users/byx/Documents/workspace/github.com/TokenFleet-AI/tokenless/crates/tokenless-schema/src/format_router.rs:21`
- `/Users/byx/Documents/workspace/github.com/TokenFleet-AI/tokenless/crates/tokenless-schema/src/format_router.rs:34`

这对 3-5 个策略非常合适，但若未来新增：
- semantic-aware structural strategy
- tool-profile-aware strategy
- cost-aware strategy
- streaming-friendly strategy

就会演变成更长的 if/else 与更多启发式冲突。缺少正式的：
- `CompressionStrategy` trait
- 策略 capability metadata
- 路由评分机制

#### 5.2 Agent 支持扩展主要靠 hook handler 手工分支

spec 写的是 11 种 agent 集成，但 CLI 中真正承载差异化协议适配的热点是 `commands/hook.rs`，它现在已经 849 行。新增 Agent 仍可能意味着：
- 新增一个协议分支
- 新增输入/输出 JSON 变体
- 修改安装器与检测逻辑

这对前几个 agent 没问题，但对长期维护不友好。更理想的是：
- `AgentHookAdapter` trait
- `InstallRecipe`
- `HookProtocolTranslator`

把 agent 差异收敛到独立模块或注册表中。

#### 5.3 统计层缺少事件模型

当前 stats 更像 record 表驱动系统，而不是统一事件流。未来若要支持：
- 压缩策略 A/B test
- 实时 dashboard 订阅
- 学习型策略回放
- 多项目、多命名空间聚合

只靠 SQLite 表查询仍能做，但会逐渐吃力。更适合补一个内部事件模型或 append-only event abstraction。

#### 5.4 语义压缩与结构压缩尚未形成统一策略管道

现在 `tokenless-semantic` 是平行能力，而不是 `tokenless-schema` 里的正式一环。这会导致：
- 调用方自己决定先结构压缩还是先语义压缩
- 组合策略语义不够明确
- 成本 / 收益评估难统一

未来如果做“智能压缩策略中心”，这两部分应该归并到一个统一策略规划器中。

---

## 6. 架构债务

### 结论

**spec 0016 所说的“main.rs 巨石”已明显改善，但真正的债务已转移到 hook 协议层与文档失配。**

### 发现

#### 6.1 `main.rs` 已经不是主要问题

spec 0016 写的是：
- `main.rs` 约 1357 行
- CLI 模块化拆分延后

但当前实测：
- `/Users/byx/Documents/workspace/github.com/TokenFleet-AI/tokenless/crates/tokenless-cli/src/main.rs` 约 **530 行**

并且 handler 已拆入：
- `commands/compress.rs`
- `commands/stats.rs`
- `commands/rewrite.rs`
- `commands/hook.rs`
- 等

所以结论很明确：**CLI 模块化拆分并非“未做”，而是“做了一半且文档没更新”。**

#### 6.2 真正的巨石是 `commands/hook.rs`

当前最大的单模块是：
- `/Users/byx/Documents/workspace/github.com/TokenFleet-AI/tokenless/crates/tokenless-cli/src/commands/hook.rs`，约 **849 行**

这说明债务并没有消失，而是迁移到了“协议适配集中点”。这类文件通常会出现：
- JSON 结构分支繁多
- Agent 差异规则散落
- 测试覆盖难以做到按适配器隔离

#### 6.3 规格文档与实现状态存在多处失配

几个典型失配：
- spec 0001 仍描述 `crates/core` / 老层次模型，但仓库里 `crates/core` 已删除
- spec 0001 未体现 `tokenless-semantic`
- spec 0016 仍把 CLI 拆分标为延后，但代码已完成主要拆分
- spec 0001 中 `tokenless-tui` 被描述为新增 crate，而当前又存在 `apps/tui` 包装器，文档未反映双层结构全貌

这类“文档债”会直接影响后续架构判断和 onboarding。

### 债务优先级排序

1. **P0：架构文档失配**
2. **P1：hook 协议模块过大**
3. **P1：策略扩展点缺少 trait/registry**
4. **P2：stats 展示与存储耦合**
5. **P2：公共 API 暴露面偏宽**

---

## 7. 新功能规划建议

以下建议基于当前架构现实，而不是理想化重写。

### 建议 1：策略注册表与评分式路由器

- **优先级**：P0
- **复杂度**：中高
- **预期影响**：高

#### 内容

将 `FormatRouter` 从固定枚举判断升级为：
- `CompressionStrategy` trait
- `StrategyScore`
- `RoutingContext`（shape、tool type、context、size、experimental flag）
- 可插拔策略注册表

#### 价值

- 新增压缩策略不再改核心 if/else
- 可把 `tokenless-semantic` 逐步接入统一策略管道
- 为未来 A/B test、策略学习、按工具类型选策略打基础

#### 触及文件

- `/Users/byx/Documents/workspace/github.com/TokenFleet-AI/tokenless/crates/tokenless-schema/src/format_router.rs`
- `/Users/byx/Documents/workspace/github.com/TokenFleet-AI/tokenless/crates/tokenless-schema/src/lib.rs`
- `/Users/byx/Documents/workspace/github.com/TokenFleet-AI/tokenless/crates/tokenless-semantic/src/lib.rs`

---

### 建议 2：Hook Adapter 插件化

- **优先级**：P0
- **复杂度**：中
- **预期影响**：高

#### 内容

把 `commands/hook.rs` 按 Agent 协议拆成：
- `claude_adapter`
- `cursor_adapter`
- `gemini_adapter`
- `copilot_adapter`
- 通用 `HookAdapter` trait

并让 `init` 安装逻辑共享同一份 agent capability metadata。

#### 价值

- 新增 Agent 的成本线性下降
- hook 逻辑测试可以按适配器独立编写
- 协议差异不再集中成一个巨石文件

#### 触及文件

- `/Users/byx/Documents/workspace/github.com/TokenFleet-AI/tokenless/crates/tokenless-cli/src/commands/hook.rs`
- `/Users/byx/Documents/workspace/github.com/TokenFleet-AI/tokenless/crates/tokenless-cli/src/init/mod.rs`

---

### 建议 3：统一压缩策略观测层（Explain / Why this strategy）

- **优先级**：P1
- **复杂度**：中
- **预期影响**：高

#### 内容

为每次压缩提供结构化 explanation：
- 选中了哪个策略
- 为什么选中
- 节省了多少 bytes/tokens
- 哪些字段被删/截断
- 是否启用 semantic / diff / TOON

可通过：
- CLI `--report json`
- MCP metadata
- TUI explain 面板

#### 价值

这会极大改善可观测性，也能为策略学习和用户信任建立基础。

---

### 建议 4：异步远程语义提供者

- **优先级**：P1
- **复杂度**：中高
- **预期影响**：中高

#### 内容

在 `tokenless-semantic` 中引入 backend 抽象：
- `RuleBased`
- `OnnxLocal`
- `RemoteEmbeddingApi`

先在 trait 层准备好，CLI 主路径仍可保持同步；对远程 provider 使用独立 feature 和 adapter。

#### 价值

- 为 spec 0010 的 Semantic-Aware Compression 真正落地扫清架构障碍
- 让未来接入托管 embedding 服务不需要重写整个 CLI

---

### 建议 5：统计事件总线与策略学习基础设施

- **优先级**：P2
- **复杂度**：高
- **预期影响**：中高

#### 内容

从“记录结果”升级到“记录决策事件”：
- `CompressionRequested`
- `StrategySelected`
- `CompressionApplied`
- `SavingsObserved`
- `FallbackTriggered`

先落 SQLite append-only 事件表即可，不必一开始引入消息系统。

#### 价值

- 为 Cross-Session Learning 与 RL policy 提供训练样本
- 更容易回答“某策略为何在某工具上效果差”

---

## 8. 架构改进路线图

## Phase 1：校准文档与拆分热点（1-2 周）

### 目标

让文档重新反映真实架构，并优先处理最大的维护热点。

### 动作

1. 更新 `specs/0001-architecture.md`
   - 去掉已失效的 `crates/core` 现实描述
   - 正式纳入 `tokenless-semantic`
   - 反映 `crates/tokenless-tui + apps/tui` 双层结构
2. 更新 `specs/0016-architecture-alignment.md`
   - 把 CLI 拆分状态改为“部分完成”或“已完成主拆分，hook 待继续拆”
3. 拆分 `commands/hook.rs`
   - 先按 agent 维度切文件，不必一次性抽象到 trait

### 成功标准

- 架构文档与代码不再互相矛盾
- 最大单文件从 849 行降到更可维护的多个子模块

---

## Phase 2：建立正式扩展点（2-4 周）

### 目标

让“新增策略”和“新增 Agent”变成插件化，而不是继续堆分支。

### 动作

1. 为压缩策略建立 trait + registry
2. 为 hook 协议建立 adapter trait + metadata
3. 为 semantic backend 建立 provider abstraction

### 成功标准

- 新增一个压缩策略不需要修改多个 match / if 链
- 新增一个 agent 主要表现为新增一个 adapter 文件

---

## Phase 3：统一观测与学习闭环（4-8 周）

### 目标

让 tokenless 从“压缩工具箱”升级为“可解释、可学习的压缩平台”。

### 动作

1. 扩展 stats 为 decision/event 记录
2. 给 CLI/MCP/TUI 增加 explain/report 输出
3. 基于历史数据做策略效果评估和推荐

### 成功标准

- 能回答“为什么这次用了这个策略”
- 能按工具/Agent/项目评估策略收益
- 为 roadmap 中 semantic learning / cross-session learning 提供基础

---

## 9. 最终判断

如果以“Rust workspace 模板项目”的标准看，tokenless 已经超过了许多同类 CLI 工具：
- 分层基本存在
- crate 边界大体合理
- 依赖方向健康
- 功能创新密度高

但如果以“可长期演进的压缩平台”标准看，它正处在一个关键拐点：
- 早期通过集中逻辑快速交付是成功的
- 现在最大挑战已从“有没有功能”转为“如何继续扩展而不失控”

一句话总结：

> tokenless 当前不是架构失控，而是已经从“单体功能产品”进入“平台化重构窗口期”。现在最该做的不是重写，而是把文档、扩展点、协议适配三件事补齐。
