# tokenless 安全审查报告（2026-06-17）

## 总体结论

**总体安全评分：7.5 / 10**

这是一个安全基线明显高于普通 Rust CLI 模板项目的代码库：
- 已完成 spec 0015 中多数供应链与 CI 加固项。
- 工作区级 `unsafe_code = "forbid"` 已开启，关键持久化层使用参数化 SQL。
- Hook / MCP 主路径基本遵循“失败不阻断、协议不污染 stdout”的设计。
- 依赖审计与 secrets 扫描已进入 CI。

但从“面向不可信 LLM/Agent 输入”的安全视角看，仍有几个重要缺口：
- 多处通过 `sh -c` 拼接命令检查依赖，仍保留不必要的 shell 注入面。
- MCP 工具参数做了“存在性读取”，但缺少统一的类型/范围/大小约束，DoS 边界主要依赖下游压缩器默认值而非入口验证。
- 统计与调试日志会持久化原始/压缩前后的文本，当前敏感信息检测是启发式，容易漏掉真实 secret。
- 路径来源虽然多为本地配置，但对 `TOKENLESS_STATS_DB`、`TOKENLESS_TOOL_READY_SPEC`、修复脚本路径等缺少 canonicalize + allowlist/ownership 约束。
- `env_check --fix` 允许执行外部脚本，属于高权限动作，但其脚本来源校验较弱。

结论：**项目已具备“可上线使用”的基础安全性，但尚未达到“默认对抗 hostile tool output / hostile local config / hostile MCP client” 的硬化水平。**

---

## 威胁矩阵

| 威胁 | 影响 | 可能性 | 当前状态 | 说明 |
|---|---|---:|---|---|
| MCP/CLI 输入导致内存或 CPU 资源消耗过高 | 高 | 中 | 部分缓解 | 有压缩深度/长度默认值，但 MCP 入口未统一限制参数范围与请求大小 |
| `sh -c` 拼接导致本地命令注入 | 高 | 中 | 未完全缓解 | `env_check` / MCP `check_cmd` 仍有 shell 拼接路径 |
| stats / debug log 持久化敏感内容 | 高 | 中 | 部分缓解 | 有启发式 `sanitize_stats_text()`，但覆盖面有限 |
| 本地配置/环境变量诱导读写非预期路径 | 中-高 | 中 | 部分缓解 | 可覆盖 stats DB、spec、fix script 路径，但缺少路径规范化/边界约束 |
| `env_check --fix` 执行恶意脚本 | 高 | 中 | 未完全缓解 | 通过候选路径选择脚本，但不校验归属、权限、哈希或目录边界 |
| MCP 工具参数类型混淆/超大整数导致异常行为 | 中 | 中 | 部分缓解 | 缺少 JSON Schema 级强校验和显式上限 |
| 依赖供应链漏洞 | 中 | 低-中 | 已较好缓解 | CI 已跑 `cargo audit`/`cargo deny`/gitleaks；当前无阻断级已知 CVE |
| 未知 secret 在日志/报错中泄露 | 高 | 中 | 部分缓解 | 避免了大量 stdout 污染，但 stderr/debug/stats 仍可能落盘 |
| 外部模型下载/网络探测被滥用 | 中 | 低-中 | 部分缓解 | 仅少数路径涉及网络，但缺少 SSRF/域名 pinning 风格约束 |

---

## 1. 输入信任边界

### 发现 1.1：CLI / Hook 主路径对 JSON 解析是严格的，但缺少统一“输入大小上限”
- `crates/tokenless-cli/src/commands/compress.rs:25`、`crates/tokenless-cli/src/commands/compress.rs:104`、`crates/tokenless-cli/src/commands/hook.rs:28`、`crates/tokenless-cli/src/commands/hook.rs:109` 都会先读取完整输入，再 `serde_json::from_str`。
- `specs/0005-security-model-design.md:49`、`specs/0005-security-model-design.md:51` 说明了深度、数组、字符串截断策略，但这些是“压缩阶段限制”，不是“请求接入阶段限制”。

影响：
- 恶意 MCP client 或 hook payload 可以先提交超大 JSON，再在压缩前消耗内存。
- 当前更像“处理后截断”，而不是“边界处拒绝”。

建议：
- 在 stdin/file 读取后、JSON 解析前增加字节上限。
- 对 MCP 单次请求长度、`tool arguments` 字符串长度、数组长度、整数范围做显式拒绝。

### 发现 1.2：MCP 工具参数存在性检查较多，但类型/范围校验不足
- `crates/tokenless-cli/src/mcp.rs:355` 仅检查 `schema` 是否存在。
- `crates/tokenless-cli/src/mcp.rs:396`、`crates/tokenless-cli/src/mcp.rs:399` 读取 `u64` 后直接转 `usize`。
- `crates/tokenless-cli/src/mcp.rs:321`、`crates/tokenless-cli/src/mcp.rs:322` 将 `arguments` 当成任意 `Value`，未校验必须为 object。

影响：
- 虽然 `as_u64()` 能过滤负数和非整数，但没有上限，理论上可传极大值，放大 CPU/内存成本。
- JSON-RPC 层未把 schema 校验前置到工具入口，工具行为依赖内部默认值和 serde 容错。

建议：
- 每个 MCP tool 增加 `validate_*_args()`，校验：
  - `arguments` 必须是 object。
  - 长度限制，例如 `truncate_strings_at <= 4096`、`truncate_arrays_at <= 256`、`max_depth <= 32`。
  - `command`、`toon`、`project`、`namespace` 等字符串字节上限。

### 发现 1.3：`tool-ready` spec 是不可信本地输入，但解析后未做字段验证
- `crates/tokenless-cli/src/env_check/spec.rs:179` 直接读取 JSON spec。
- `crates/tokenless-cli/src/env_check/spec.rs:48` 的 `normalize_dep()` 对缺字段对象会生成空字符串字段。
- `crates/tokenless-cli/src/env_check/mod.rs:88` 后续会把这些字段交给检查和修复逻辑。

影响：
- 篡改 spec 虽不会直接造成远程代码执行，但会改变依赖检查和修复行为。
- 空 binary / 异常 manager / 异常 fallback method 进入后续流程，会产生不可预测行为或扩大攻击面。

建议：
- 加入 spec schema 校验：binary/package/manager 非空；允许值枚举；fallback method allowlist。
- 对 `required/recommended/config_files/permissions/network` 做数量上限。

小结：**输入验证方向基本正确，但目前更偏“处理时容错”，还不算“边界即拒绝”。**

---

## 2. 路径安全

### 发现 2.1：stats/config/spec/fix 脚本路径支持环境变量覆盖，但缺少边界约束
- Stats DB 路径来自 `TOKENLESS_STATS_DB`：`crates/tokenless-cli/src/shared.rs:127`、`crates/tokenless-cli/src/mcp.rs:65`。
- tool-ready spec 路径来自 `TOKENLESS_TOOL_READY_SPEC`：`crates/tokenless-cli/src/env_check/spec.rs:263`。
- fix script 路径来自 `TOKENLESS_ENV_FIX_SCRIPT`：`crates/tokenless-cli/src/env_check/fixer.rs:140`。

当前情况：
- 会 `create_dir_all(parent)`，但不会检查符号链接、canonical path、是否落在允许目录。
- 对“本地恶意环境变量/受污染启动环境”没有防护。

影响：
- 本地攻击者可诱导写入任意用户可写路径，例如伪装 DB、报告目录、debug log。
- 若 fix script 指向恶意脚本，则 `env_check --fix` 将直接执行。

建议：
- 对可执行/可写路径做 `canonicalize()` 后校验：
  - 默认仅允许用户 home 下 `.tokenfleet-ai/tokenless/`。
  - 如允许覆盖，则要求 `--allow-external-paths` 之类显式开关。
- 对脚本路径额外校验：非 world-writable、owner 为当前用户、不是目录、扩展/文件名匹配预期。

### 发现 2.2：`init` 会写多类 agent 配置文件，但未见权限收紧
- `crates/tokenless-cli/src/init/mod.rs:215` 的 `write_file()` 使用 `fs::File::create` 默认权限。
- 会写入 `.claude/settings.json`、hooks 脚本、规则文件等。

影响：
- 在多用户共享环境，默认权限取决于 umask；敏感配置文件可能过宽。
- Hook 脚本虽然不含 secret，但会影响 agent 行为，属于安全相关文件。

建议：
- Unix 下对脚本/配置文件显式设置权限：配置 `0600/0644`，脚本 `0755`。
- 写前检查目标是否为 symlink，避免被重定向覆盖。

### 发现 2.3：配置/日志目录默认在用户目录，方向正确，但未做 symlink 防护
- `crates/tokenless-cli/src/shared.rs:115`、`crates/tokenless-cli/src/shared.rs:133` 创建 tokenless 工作目录与 reports 目录。
- `crates/tokenless-cli/src/commands/hook.rs:283` 直接追加写 `compress-debug.log`。

影响：
- 如果攻击者能预先布置 symlink，可导致意外写入其他文件。

建议：
- 对日志、DB、reports 路径增加 `symlink_metadata` 检查。

小结：**没有明显的传统 `../` 路径遍历，但“环境变量/符号链接/任意绝对路径覆盖”仍是现实风险。**

---

## 3. 密钥 / Token 处理

### 发现 3.1：未发现硬编码 secret，但“响应内容可能包含 secret”是核心风险
- 仓库搜索未发现明显硬编码凭证。
- 但 threat model 已承认 API response data 可能包含 PII / secrets：`specs/0005-security-model-design.md:10`。
- 压缩、hook、stats 都会处理这些文本。

结论：
- 风险不在“代码内置 secret”，而在“工具输出中的 secret 被记录/打印/存盘”。

### 发现 3.2：stats 文本脱敏仅是启发式，容易漏报
- `crates/tokenless-stats/src/recorder.rs:663` 的 `sanitize_stats_text()` 只检查：
  - `Bearer ` + 长值
  - `Authorization`
  - `api_key` / `apikey` / `token`
- 它不会识别很多真实场景：
  - `password=`、`secret=`、`client_secret=`、`refresh_token=`
  - GitHub PAT、AWS AKIA、Slack token、JWT、私钥 PEM
  - JSON 中嵌套字段、Base64-like 长串、cookie/session 值

影响：
- `before_text` / `after_text` 很可能仍把敏感数据写入 `stats.db`。

证据：
- 记录逻辑在 `crates/tokenless-cli/src/shared.rs:168` 接收完整 `before_text` / `after_text`。
- 落库在 `crates/tokenless-stats/src/recorder.rs:177`。

### 发现 3.3：debug log 直接记录 before/after 文本，未见敏感检测
- `crates/tokenless-cli/src/commands/hook.rs:276` 的 `write_debug_log()` 会把 `before` / `after` 写入 `compress-debug.log`。
- 虽然截断到 4096 字符，但并未调用 `sanitize_stats_text()` 或其他 secret redaction。

影响：
- 打开 `--debug` 的用户，最容易把真实工具输出（例如 `env`, `kubectl`, `aws`, `gh auth` 返回）落盘。

### 发现 3.4：stderr 诊断有少量内容泄露面
- `crates/tokenless-cli/src/shared.rs:219` 会输出 `record_compression_stats SKIP: at=... op=...`。
- 这里不含 payload，本身问题不大。
- 但未来如果更多错误上下文直接拼接原文本，就会形成泄露面，建议建立统一 redaction 层。

### `secrecy` 集成必要性评估
spec 0015 将 `secrecy` 标为未完成项：`specs/0015-security-hardening.md:245`。

我认为**有必要集成，但应集中在“持久化前与日志前的敏感载荷封装”，不是全项目泛化包裹所有字符串。**

优先包装字段：
1. `before_text` / `after_text` 对应的原始工具输出与压缩输出。
2. debug log 中的 `before` / `after`。
3. 未来若实现 stats export / backup，应将导出内容中的文本字段视为 secret-bearing data。
4. 若引入远程下载认证、私有 MCP、API key 配置，再包装对应 token / header 字段。

不必优先包装的字段：
- `project`、`namespace`、`agent_id`、`tool_name`、`session_id`，它们更偏标识符，不一定是 secret。

推荐做法：
- 在 `StatsRecord` 中将文本敏感载荷改为 `SecretString` 或内部 redacted newtype。
- 为展示/导出场景显式 `expose_secret()`，默认 `Debug` 全部打码。
- debug log 改成默认只写摘要，除非显式 `--debug-unsafe-include-payloads`。

小结：**当前最大安全短板之一就是“敏感文本持久化治理不足”。**

---

## 4. MCP Server 安全

### 发现 4.1：JSON-RPC 协议面基本规范，但工具层缺少细粒度参数治理
- `crates/tokenless-cli/src/mcp.rs:120` 对请求做 JSON 解析。
- `crates/tokenless-cli/src/mcp.rs:136` 校验 `jsonrpc == "2.0"`。
- `crates/tokenless-cli/src/mcp.rs:152` 忽略 notification，避免无意义响应。

优点：
- stdout 只写 JSON-RPC 响应，协议污染风险较低。
- unknown method / parse error 都能返回标准错误对象。

不足：
- `tools/call` 只靠 `name` 分发，未对 `arguments` 做对象约束与 schema 验证：`crates/tokenless-cli/src/mcp.rs:321`。

### 发现 4.2：`env_check` 在 MCP 中仍暴露 shell 拼接检查路径
- `crates/tokenless-cli/src/mcp.rs:87` 的 `check_cmd()` 使用：
  `Command::new("sh").args(["-c", &format!("command -v {cmd}")])`
- `cmd` 可直接来自 MCP `env_check` 的 `tool` 参数：`crates/tokenless-cli/src/mcp.rs:499`。

这是本次审查中最明确的代码级风险点之一。

分析：
- 如果传入 `tool = "bash; touch /tmp/pwned"`，shell 会解释分号。
- 这不是“理论上危险”，而是典型命令拼接模式。
- 虽然这里用途只是检查 PATH，但已经把不可信 MCP 参数送进 shell。

### 发现 4.3：`env_check/checker.rs` 的 `check_cmd()` 同样存在 shell 调用历史包袱
- `crates/tokenless-cli/src/env_check/checker.rs:9` 也使用 `sh -c`。
- 不过 `check_dep()` 已做得更安全：`crates/tokenless-cli/src/env_check/checker.rs:112` 通过 `command -v "$1" -- <binary>` 传参，注入面显著更小。

建议：
- 统一移除 `sh -c` 检查路径，改用纯 argv 形式或直接手写 PATH 搜索。
- 最佳方案是实现一个本地 `is_executable_on_path(name: &str)`，只接受 allowlist 字符集 `[A-Za-z0-9._+-]`。

### 发现 4.4：MCP 返回值把 result 包进 `content[0].text`，协议兼容但容易丢失结构化边界
- `crates/tokenless-cli/src/mcp.rs:328` 将工具返回 JSON 再序列化成 text。

这本身不是漏洞，但带来两个问题：
- 客户端若误把 text 当自然语言，可能二次解析失败。
- 未来如果 text 内包含过大 JSON，仍会造成上下文膨胀。

建议：
- 若协议允许，优先返回结构化 JSON content，而不是 JSON string inside text。
- 至少给每个工具结果增加大小上限与截断策略。

小结：**MCP server 协议面不错，但 `env_check` 的 shell 拼接属于应尽快修复的 P1 级问题。**

---

## 5. 依赖供应链安全

### 当前状态
- `deny.toml` 已设置：
  - `wildcards = "deny"`：`deny.toml:160`
  - `unknown-registry = "deny"`：`deny.toml:210`
  - `unknown-git = "deny"`：`deny.toml:213`
  - 禁止 `openssl-sys`：`deny.toml:182`
  - 禁止 `reqwest` 的 native-tls 特性：`deny.toml:200`
- CI 已执行：
  - `cargo deny check`：`.github/workflows/ci.yml`
  - `cargo audit`：`.github/workflows/ci.yml`
  - `gitleaks`：`.github/workflows/ci.yml`
- workflow 权限已经最小化：`.github/workflows/ci.yml`、`.github/workflows/release.yml`

### 实际审计结果
本地运行结果显示：
- `cargo audit` 仅有 1 条**已允许的 warning**：`paste 1.0.15` / `RUSTSEC-2024-0436`，来自 `tokenizers`，属于 unmaintained advisory，不是直接 exploitable CVE。
- `cargo deny check advisories bans sources` 通过。
- `cargo deny` 还有 duplicate crate 警告（如 `hashbrown` 多版本），但这属于体积/维护性问题，不是直接安全漏洞。

### 发现 5.1：可选 ONNX / tokenizers 路径是当前最值得重点盯防的供应链面
- `Cargo.toml` 工作区使用 `tokio`、`rusqlite`、`regex`、`tracing` 等主流库，风险可控。
- `crates/tokenless-semantic/Cargo.toml` 启用了可选 `tokenizers`、`ort`、`ureq` 路径；其中 `tokenizers` 已引出 advisory ignore。

建议：
- 将 `tokenless-semantic` 相关依赖作为重点升级监控对象。
- 如果语义压缩不是默认关键路径，可考虑把 ONNX 能力拆到更隔离的 feature / crate，减少默认安装面的依赖压力。

### 发现 5.2：依赖数偏多，但“真正高风险依赖”集中在少数功能块
你给出的依赖规模约 276 个，结合工作区结构看：
- 核心 CLI/Schema/Stats 的风险相对可控。
- 扩张主要来自 TUI 和 semantic/ONNX 相关栈。

建议裁剪候选：
1. `tokenless-semantic` 默认是否必须存在于主 workspace 发行路径。
2. TUI 与 CLI 是否可进一步隔离 feature/发布物。
3. `chrono`、`dirs`、`toml` 等是否在所有 crate 都必要。

小结：**供应链姿态整体良好，当前不是主要短板；需要重点关注 optional semantic 栈。**

---

## 6. `secrecy` 集成必要性

### 结论
**必要，且建议列为 P1。**

原因不是“项目里有很多 API key 配置字段”，而是：
- tokenless 的业务模型决定它会接触“来自工具输出的高敏文本”。
- 这些文本当前会进入 stats、debug log、可能未来的 export/backup。
- 只靠字符串模式匹配不足以保证不泄露。

### 推荐优先包裹的字段/类型
1. `StatsRecord.before_text`
2. `StatsRecord.after_text`
3. compress debug log 的 `before` / `after`
4. 未来 report/export/backup 中的原始文本字段
5. 若以后支持 MCP auth / remote service token，再包裹认证字段

### 推荐设计
- 新增 `SensitiveText(SecretString)` newtype：
  - `Debug` 默认打码
  - `Display` 禁止直接暴露
  - 仅在显式持久化/导出路径通过受控方法解包
- `StatsRecord` 中将“原始文本存储”从默认行为改为：
  - 默认关闭或仅存摘要
  - 显式开启 `store_text_payloads = true` 时才持久化
- 在 recorder 层做结构化 redaction，而非纯字符串关键字搜索

### 为什么不是 P2
因为当前真实暴露面已经存在：
- `stats.db`
- `compress-debug.log`
- 未来的 backup/export 规格

这不是锦上添花，而是针对现有数据面风险的补洞。

---

## 7. 新功能与安全改进建议（3-5 项）

### 建议 A：统一输入校验层 `ValidatedMcpArgs` / `ValidatedCliInput`
- 优先级：P1
- 复杂度：中
- 预期影响：高

内容：
- 为 MCP 每个工具增加独立参数校验函数。
- 统一限制请求字节数、字符串长度、数组长度、深度、整数上限。
- 对 `env_check` spec 解析引入 schema 校验。

收益：
- 明确 trust boundary。
- 大幅降低 DoS 和类型混淆类问题。

### 建议 B：移除所有 `sh -c` 依赖检查，改为纯 Rust PATH 查找
- 优先级：P1
- 复杂度：低-中
- 预期影响：高

内容：
- 替换 `crates/tokenless-cli/src/mcp.rs:87` 与 `crates/tokenless-cli/src/env_check/checker.rs:9` 等路径。
- 只接受安全字符集的 binary 名称。
- 尽量不经 shell。

收益：
- 直接消除最明确的本地命令注入面。

### 建议 C：敏感文本治理升级（默认摘要化 + `secrecy` + redaction engine）
- 优先级：P1
- 复杂度：中-高
- 预期影响：很高

内容：
- stats 默认只存长度、token、hash、类别，不存完整 before/after。
- 提供显式开关启用原文落盘，并在 UI/CLI 强提醒。
- 引入 `secrecy`，统一 redacted debug。
- 新增更强的 secret detector：JWT、PAT、AWS key、PEM、cookie、password、client_secret 等。

收益：
- 降低最严重的数据泄露面。
- 更适合企业/团队环境落地。

### 建议 D：路径安全加固（canonicalize + owner/permission/symlink 检查）
- 优先级：P1
- 复杂度：中
- 预期影响：中-高

内容：
- 对 stats DB、reports、spec、fix script、debug log 路径增加：
  - canonical path
  - symlink 拒绝
  - owner/permission 检查
  - 默认目录 allowlist
- 对 `env_check --fix` 增加“脚本信任验证”。

收益：
- 防止本地环境污染导致的任意文件读写/执行偏转。

### 建议 E：安全模式开关 `tokenless --secure-default`
- 优先级：P2
- 复杂度：中
- 预期影响：高

内容：
- 一键启用：
  - 禁止原文 stats 持久化
  - 禁止 debug payload 落盘
  - 限制外部路径覆盖
  - 禁止 `env_check --fix` 执行非受信脚本
  - MCP 请求大小上限

收益：
- 给普通用户一个“安全默认配置”，降低误配置概率。

---

## 8. 安全路线图

### 立即（1-2 周）
1. 修复 `sh -c` 注入面：
   - `crates/tokenless-cli/src/mcp.rs`
   - `crates/tokenless-cli/src/env_check/checker.rs`
2. MCP 入口增加参数类型与大小校验。
3. `write_debug_log()` 默认禁用 payload 原文，至少先接入现有 sanitizer。
4. `env_check --fix` 对脚本路径增加 canonicalize + symlink/owner 检查。

### 近期（2-4 周）
1. 引入 `secrecy`，重构 stats/debug payload 类型。
2. stats 默认改为“摘要优先，原文可选”。
3. 为 spec/fix script/path override 建立统一路径安全工具模块。
4. 增加 secret redaction 测试集与恶意输入测试集。

### 中期（1-2 个月）
1. 设计并实现 `secure-default` 模式。
2. 把 optional semantic / model download 路径进一步隔离。
3. 为 MCP 加入请求大小限制、速率限制或会话级资源预算。
4. 提供 stats export / backup 的加密或最少 redaction 方案。

---

## 分维度详细结论

### 1）输入信任边界
- 结论：**中等偏好，但未达强边界验证**
- 主要问题：缺少统一请求大小与参数范围限制。

### 2）路径安全
- 结论：**无传统遍历问题，但存在路径覆盖/符号链接/脚本信任缺口**
- 主要问题：环境变量覆盖路径缺少安全约束。

### 3）密钥/Token 处理
- 结论：**没有硬编码 secret，但文本敏感数据持久化风险较高**
- 主要问题：stats/debug 的敏感信息治理不足。

### 4）MCP Server 安全
- 结论：**协议层较稳，工具层校验不足**
- 主要问题：`env_check` 的 shell 拼接与参数缺少范围验证。

### 5）依赖供应链安全
- 结论：**整体较好**
- 主要问题：可选 semantic 栈需持续监控，存在允许的 unmaintained advisory。

### 6）`secrecy` 集成必要性
- 结论：**必要，且不应继续拖到 P2 以后**
- 主要问题：业务决定了该项目天然会接触高敏输出文本。

---

## 最关键的 5 条发现摘要

1. **P1：MCP `env_check` 可通过 `sh -c` 处理不可信 `tool` 参数，存在命令注入面。**
2. **P1：stats 与 debug log 会落盘原始工具输出，当前敏感检测过于启发式。**
3. **P1：路径覆盖（DB/spec/fix-script）缺少 canonicalize、symlink、owner 边界校验。**
4. **P1：MCP / CLI 缺少统一请求大小与参数范围治理，DoS 面仍偏宽。**
5. **P2：供应链总体良好，但 `tokenless-semantic` optional 栈是主要监控热点。**

---

## 参考证据文件

- `specs/0005-security-model-design.md`
- `specs/0015-security-hardening.md`
- `specs/0006-error-handling-strategy.md`
- `crates/tokenless-cli/src/mcp.rs`
- `crates/tokenless-cli/src/env_check/mod.rs`
- `crates/tokenless-cli/src/env_check/checker.rs`
- `crates/tokenless-cli/src/env_check/fixer.rs`
- `crates/tokenless-cli/src/env_check/spec.rs`
- `crates/tokenless-cli/src/commands/hook.rs`
- `crates/tokenless-cli/src/commands/compress.rs`
- `crates/tokenless-cli/src/shared.rs`
- `crates/tokenless-stats/src/recorder.rs`
- `.github/workflows/ci.yml`
- `deny.toml`
