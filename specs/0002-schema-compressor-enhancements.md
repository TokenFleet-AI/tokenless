# 0002 — Schema & Response Compressor

> 压缩器完整设计：已实现原理 + 三项规划增强。

---

## Part A: SchemaCompressor（已实现）

### A.1 设计目标

压缩 OpenAI Function Calling 工具定义，在 Tool Schema 进入 LLM 上下文窗口前减少约 57% 的结构性 Token 开销。

### A.2 核心原理

**Builder 模式 + 零节省回退**：

```rust
pub struct SchemaCompressor {
    func_desc_max_len: usize,   // 函数级描述最大字符数（默认 256）
    param_desc_max_len: usize,  // 参数级描述最大字符数（默认 160）
    drop_examples: bool,        // 移除 examples（默认 true）
    drop_titles: bool,          // 移除 title（默认 true）
    drop_markdown: bool,        // 剥离 Markdown 格式（默认 true）
}
```

**压缩入口** `compress(&self, tool: &Value) -> Value`：

```
1. 序列化原始 tool → original_text
2. 深度克隆 tool → result
3. 判断 tool 结构：
   ├── 有 "function" 包装键 → 进入 function 内部压缩
   └── 无 "function" 包装键 → 直接压缩裸 schema（兼容 JSON Schema 直传）
4. 序列化压缩后 result → compressed_text
5. 比较 original_text vs compressed_text
   ├── 相同 → 返回原始 tool.clone() （零节省，不浪费）
   └── 不同 → 返回压缩后 result
```

**为什么比较序列化字符串而非结构**：JSON 对象引用比较不可靠，`serde_json::Value` 的 `==` 比较会逐字段递归。序列化为字符串后比较，简单且能捕获所有差异（空格、顺序等）。代价是两次序列化（见 0009-optimization-analysis 中的优化建议）。

### A.3 递归 JSON Schema 压缩

`compress_json_schema(&self, schema: &mut Value, depth: usize)` 原地修改：

```
对每个 schema 节点：
  ├── 移除 title（如果 drop_titles）
  ├── 移除 examples（如果 drop_examples）
  ├── 截断 description：
  │   ├── depth == 0 → func_desc_max_len (256)
  │   └── depth >= 1 → param_desc_max_len (160)
  └── 递归进入子节点：
      ├── properties.* → compress_json_schema(depth+1)
      ├── items → compress_json_schema(depth+1)
      ├── anyOf[*] → compress_json_schema(depth+1)
      ├── oneOf[*] → compress_json_schema(depth+1)
      └── allOf[*] → compress_json_schema(depth+1)
```

**支持的 JSON Schema 关键字**：`properties`, `items`, `anyOf`, `oneOf`, `allOf`

**当前不支持**（在 OpenAI Tools 中极少出现）：`$ref`, `$defs`, `additionalProperties`, `patternProperties`, `if`/`then`/`else`

### A.4 句子边界感知截断

`truncate_description(&self, desc: &str, max_len: usize) -> String`：

```
Step 1: 预处理
  ├── 移除 Markdown 代码块（```...```）
  ├── 移除行内代码（`...`）
  └── 合并连续空白 → 单个空格

Step 2: 长度检查
  └── chars.count() <= max_len → 直接返回（无需截断）

Step 3: 句子边界搜索
  ├── 搜索范围：[max_len * 0.5, max_len]
  ├── 在范围内查找最后一个句子结束符（. 。！ ？）
  ├── 找到 → 按句子边界截断
  └── 未找到 → 硬截断到 max_len（char 边界安全）

Step 4: 字符边界安全
  └── find_char_boundary() 确保不在多字节 UTF-8 字符中间截断
```

**为什么是 `[max_len * 0.5, max_len]` 范围**：下限 50% 防止在一个很短的句子后就截断（丢失太多信息），上限保证不超过限制。在这段范围内找最后一个句号，做到"尽量长但不超过限制"。

**CJK 处理**：CJK 文本没有空格分词，`。！？` 同样作为句子边界。纯 CJK 文本（无句号）走硬截断路径，`find_char_boundary()` 确保安全性。

### A.5 静态正则（LazyLock）

```rust
static CODE_BLOCK_RE:  LazyLock<Regex> = ...;  // ```...```
static INLINE_CODE_RE: LazyLock<Regex> = ...;  // `...`
static WHITESPACE_RE:   LazyLock<Regex> = ...;  // \s+
```

使用 `LazyLock` 而非 `lazy_static`（Rust 2024 标准库），正则编译一次全局复用。

### A.6 测试覆盖

| 测试 | 覆盖场景 |
|------|---------|
| `test_compress_long_description` | 函数级 256 + 参数级 160 双限截断 |
| `test_protected_fields_preserved` | name/type/required/enum/default/const 原样保留 |
| `test_title_and_examples_removed` | title + examples 在所有嵌套层级被移除 |
| `test_empty_schema_no_panic` | `{}` / `null` / `{"function": {}}` 不 panic |
| `test_nested_properties_recursive_compression` | 三级嵌套 properties 递归压缩 |
| `test_truncate_at_sentence_boundary` | 句子边界截断（以句号结尾） |
| `test_markdown_removal` | Markdown 代码块 + 行内代码剥离 |
| `test_anyof_oneof_allof_compression` | anyOf/oneOf/allOf 内每个分支递归压缩 |
| `truncate_description_cjk_no_panic` | 100 个 "中" 字截断不 panic，输出仍是有效 UTF-8 |
| `test_no_change_returns_original` | 零节省时返回原始对象引用 |

---

## Part B: ResponseCompressor（已实现）

### B.1 设计目标

压缩 API / Tool 返回的 JSON 响应，去除调试信息、截断长内容、限制嵌套深度。节省 26-78% Token。

### B.2 核心原理

```rust
pub struct ResponseCompressor {
    drop_fields: HashSet<String>,    // 默认 8 个调试字段
    truncate_strings_at: usize,      // 512 字符
    truncate_arrays_at: usize,       // 16 项
    drop_nulls: bool,                // true
    drop_empty_fields: bool,         // true
    max_depth: usize,                // 8 层
    add_truncation_marker: bool,     // true
}
```

### B.3 压缩规则（按优先级）

**规则 1 — 调试字段清除**（不可配置优先级，始终先执行）：
```
默认列表: debug, trace, traces, stack, stacktrace, logs, logging
自定义: add_drop_field("custom_debug_field")
```
在对象遍历时直接 `continue` 跳过，不进入后续处理。

**规则 2 — 深度保护**：
```
depth > max_depth (8) → 替换为 "<{type} truncated at depth {depth}>"
```
防止递归爆栈，同时给 LLM 一个语义提示（知道这里还有数据但被截断了）。

**规则 3 — 数组截断**：
```
len > 16 → 保留前 16 项 + "<... {remaining} more items truncated>"
遍历时同时应用 drop_nulls 和 drop_empty_fields
```

**规则 4 — 字符串截断**：
```
chars > 512 → 截断 + "… (truncated)" 标记
char_indices().nth(512) 定位字节偏移，保证 UTF-8 安全
```

**规则 5 — Null 清除**：
```
drop_nulls == true → 跳过 null 值，不加入输出
```

**规则 6 — 空字段清除**：
```
drop_empty_fields == true → 跳过 "" / [] / {}
```

### B.4 递归遍历策略

```
compress_value(value, depth):
  match value:
    Null       → Null（后续被 drop_nulls 过滤）
    Bool/Number → 直接返回（无需压缩）
    String     → compress_string() → 规则 4
    Array      → compress_array()
                  ├── 限制长度（规则 3）
                  ├── 每个元素 compress_value(depth+1)
                  └── 过滤 null/empty（规则 5/6）
    Object     → compress_object()
                  ├── 过滤 drop_fields 中的键（规则 1）
                  ├── 每个值 compress_value(depth+1)
                  └── 过滤 null/empty（规则 5/6）
```

### B.5 零节省回退

与 SchemaCompressor 相同模式（见 A.2）。注意：非 JSON 输入（如纯文本命令输出）不会触发压缩器——这由 CLI 层的 JSON parse 守卫处理。

### B.6 测试覆盖

| 测试 | 覆盖场景 |
|------|---------|
| `test_string_truncation` | 自定义截断长度 + 标记 |
| `test_string_truncation_512_default` | 默认 512 上限 |
| `test_array_compression` | 数组截断 + 剩余计数 |
| `test_drop_fields` | 全部 8 个默认字段被移除 |
| `test_drop_nulls` / `test_drop_nulls_disabled` | null 清除开关 |
| `test_drop_empty_fields` | 空字符串/数组/对象清除 |
| `test_max_depth_truncation` | 超过 8 层替换为类型标记 |
| `test_nested_object_recursive_compression` | 嵌套对象中字符串+null 同时处理 |
| `test_preserve_primitives` | bool/number/short-string 不变 |
| `test_utf8_safe_truncation` | CJK 截断仍是有效 UTF-8 |
| `test_no_change_returns_original` | 零节省回退 |

---

## Part C: 三项规划增强（未实现）

> 基于 agent-proxy-rust 压缩缺口集群分析（compress-gaps）。

### C.1 P1: max_enum_items — enum 数组截断

**问题**：长 `enum` 数组（语言代码列表、MIME types、文件扩展名）单个字段可占 500-1000 tokens，当前完全不处理。

**方案**：

```rust
pub struct SchemaCompressor {
    // ... existing fields ...
    max_enum_items: usize,  // default: usize::MAX（不限制，向后兼容）
}
```

截断策略：保留前 N 个元素，追加 placeholder：

```json
// max_enum_items=20, 原 enum 有 50 个元素
["python","javascript",...,"ruby", "<... 30 more items>"]
```

**新增 API**：`with_max_enum_items(usize)`

---

### C.2 P2: Token-aware Description Truncation — 双限保护

**问题**：当前 `truncate_description` 用字符数硬截断。中文字符约 1 token/char，英文约 0.25 token/char，同一字符限制下实际 Token 数差 4 倍。

**方案**：

1. **max_chars** 硬上限（保持现有逻辑，防止极端文本和 estimator 失效）
2. **max_tokens** 软限制（char 截断后，用改进的 token estimator 二次截断）

```rust
pub struct SchemaCompressor {
    // ... existing fields ...
    func_desc_max_tokens: usize,   // default: usize::MAX
    param_desc_max_tokens: usize,  // default: usize::MAX
}
```

**Token 估算改进** — CJK 感知：

```rust
fn estimate_tokens(text: &str) -> usize {
    let mut tokens = 0usize;
    let mut ascii_run = 0usize;
    for ch in text.chars() {
        if is_cjk(ch) {
            tokens += ascii_run.div_ceil(4);  // 英文 4 chars/token
            ascii_run = 0;
            tokens += 1;                       // CJK 1 char/token
        } else {
            ascii_run += 1;
        }
    }
    tokens += ascii_run.div_ceil(4);
    tokens
}
```

**执行顺序**：

```
truncate_description(text):
  1. 去 markdown + 合并空白（保持现有逻辑）
  2. 按 max_chars 截断（硬上限，保持现有逻辑）
  3. 如果 estimate_tokens(result) > max_tokens：
     → 按 token 预算反向缩 char，直到 estimate_tokens ≤ max_tokens
```

**新增 API**：`with_func_desc_max_tokens(usize)`, `with_param_desc_max_tokens(usize)`

---

### C.3 P3: $ref/$defs 递归压缩

**问题**：`compress_json_schema` 只递归 `properties/items/anyOf/oneOf/allOf`。遇到 `$ref` 跳过整个引用子树，`$defs` 内的 description/title/examples 全部漏掉。

**方案 — 入口收集 + 递归 resolve**（不改结构，只追踪引用）：

```rust
fn compress(&self, tool: &Value) -> Value {
    let defs = collect_defs(tool);  // $defs → HashMap<String, &Value>
    let mut visited = HashSet::new();
    self.compress_tool(tool, &defs, &mut visited)
}
```

`compress_json_schema` 新增处理：

| 关键字 | 处理方式 |
|--------|---------|
| `$ref` | 解析路径 → 从 defs 查找 → visited 检查防循环 → 递归 compress_json_schema |
| `additionalProperties` | 值为 schema object 时递归 |
| `$defs` / `definitions` | 遍历每个 entry 递归压缩 |
| `patternProperties` | 值不为空时递归每个 pattern 下的 schema |

循环引用用 `HashSet<String>`（记录已访问的 `$ref` 路径）跳过。

**不变更**：
- 不展开 `$ref` — 保持引用结构，不膨胀输出
- `propertyNames` 暂不支持（极少使用）
- `if`/`then`/`else` 暂不支持（在 OpenAI tools 中几乎不出现）

---

## 兼容性保证

- 所有新增字段有合理默认值（P1/P2 用 `usize::MAX` 表示"不限制"）
- `Default` / `new()` 行为不变，现有调用方无需改动
- 零节省回退逻辑不变：压缩无效果 → 返回原始值

---

## Part D: RuFlo 集群审查报告

> 2026-05-27，启用 RuFlo 集群（swarm-1779849748279-9wp2kh, hierarchical-mesh, 8 agents），
> 由 `architecture` Agent 审查 SchemaCompressor，`security-manager` Agent 审查 ResponseCompressor。

### D.1 SchemaCompressor 审查结论：可行，无严重缺陷

| 检查项 | 结果 | 说明 |
|--------|------|------|
| 零节省回退 | ✅ 正确 | `serde_json::Map` 底层 `BTreeMap`，序列化 Key 顺序确定，字符串比较可靠 |
| UTF-8 安全 | ✅ 正确 | `find_char_boundary()` 保证不在多字节编码中间截断。Emoji/连字符/RTL 在 Code Point 级安全（Grapheme Cluster 级会断但仍是合法 UTF-8） |
| 无限递归 | ✅ 安全 | `serde_json::Value` 是所有权树，无 `Rc`/`Arc` 无法构造循环。`$ref` 不跟随 |
| 句子边界算法 | ✅ 正确 | `max_len=0/1` 等极端值全部处理，无除零、无越界、无空范围 panic |
| Markdown 正则 | ✅ 安全 | 非贪婪匹配，畸形输入不会 panic，不会错误跨区匹配 |
| if/else 重复 | ⚠️ Warning | `compress()` 中 function-wrapper 路径和 bare-schema 路径 ~80% 重复代码，bare 路径还额外做了一次 `compress_json_schema`（对已处理字段的二次处理，无害但浪费）。新增压缩步骤需改两处，容易遗漏 |

**额外发现**：

| # | 严重度 | 说明 |
|---|--------|------|
| D1 | Info | `compress_json_schema` 是 `pub fn` 暴露了内部 `depth` 参数。外部以 `depth=0` 调用会错误使用 `func_desc_max_len` 而非 `param_desc_max_len`。建议改为 `pub(crate)` 或新增封装方法 |
| D2 | Info | 缺少递归深度上限保护。恶意 10000 层嵌套 schema 可能导致栈溢出。实践中 OpenAI Tools 极少超过 20 层，但建议加 `depth > 64 → return` 防护 |
| D3 | Info | `to_string().unwrap_or_default()` 在序列化失败时静默返回 `""`，吞掉了错误信号。建议至少 `tracing::warn!` |

### D.2 ResponseCompressor 审查结论：可行，无严重缺陷

| 检查项 | 结果 | 说明 |
|--------|------|------|
| 深度标记安全性 | ✅ 正确 | `<type truncated at depth N>` 是非空字符串，不会被 `drop_nulls`/`drop_empty_fields` 误删 |
| Null/false/0 语义 | ✅ 正确 | `is_empty_value` 对 Number/Bool 返回 false，`0` 和 `false` 是有意义的值，不应被当作"空" |
| 死代码 `map_or` | ℹ️ Info | `compress_string` 中 `char_indices().nth().map_or(s.len(), ...)` 的 fallback 永不触发（调用前已确认 len > limit）。且字符串被迭代两次（count + nth） |
| NaN/Infinity | ℹ️ Info | `serde_json::to_string` 对 `NaN`/`Infinity` 会失败，`unwrap_or_default()` 静默退化。实践中不会出现 |

**Warning 级别问题**：

| # | 严重度 | 问题 | 建议 |
|---|--------|------|------|
| D4 | ⚠️ Warning | 数组截断计数偏高：marker 说 `<... X more items truncated>`，但 X 是未遍历的原始项数，其中可能包含本来就会被 null/empty 过滤掉的项 | 改为 `<... up to X more>` 或在截断后缀上也执行过滤后再计数 |
| D5 | ⚠️ Warning | drop_fields 大小写敏感：`"Debug"`（首字母大写）不会被匹配。API 中 PascalCase 的调试字段很常见 | 文档化"大小写敏感"，或增加 `with_drop_fields_case_insensitive` 选项，默认保持 case-sensitive |
| D6 | ⚠️ Warning | Grapheme Cluster 截断：`char_indices().nth(N)` 在 Code Point 边界截断，但 "café" 中的 "é"（e + combining accent）可能在 e 和 accent 之间断开，变成 "cafe"（语义改变） | 如需完整 Grapheme 支持，引入 `unicode-segmentation` crate |
| D7 | ⚠️ Warning | Builder 模式不一致：`add_drop_field(&mut self)` vs 其他 `with_*(mut self) -> Self`，打断链式调用 | 改为 `with_drop_field(mut self, field) -> Self` |
| D8 | ⚠️ Warning | 无广度限制：500,000 个 Key 的单层 JSON 会全量处理（线性 O(n) 但无上限保护）。`max_depth=8` 只防深度不防广度 | 增加 `max_keys_per_object` 上限或文档化"调用方应限制输入大小" |
| D9 | ⚠️ Warning | 极端情况数组只剩 marker：`[null,null,...,1,2]` 前 8 个 null 全部被 drop，结果只有 `["<... 2 more items truncated>"]`，丢失了数组包含什么类型数据的上下文 | 考虑在全部 prefix 为空时追加额外说明，如 `"<all N items null/empty, plus X truncated>"` |

### D.3 总体评估

```
SchemaCompressor:   ✅ 可行 | 0 Critical | 1 Warning | 3 Info
ResponseCompressor: ✅ 可行 | 0 Critical | 6 Warnings | 2 Info
```

**核心逻辑正确，无安全/数据丢失级别缺陷。** 两项 Warning（D4 数组截断计数、D7 Builder API 不一致）建议在下一个版本修复。其余 Info/Warning 属于增强性改进。

### D.4 修复优先级

| 优先级 | 编号 | 问题 | 影响 |
|--------|------|------|------|
| **P0** | — | 无 Critical 问题 | — |
| **P1** | D7 | Builder API 不一致 | 调用方无法链式调用 `add_drop_field` |
| **P1** | D1 | `compress_json_schema` pub 暴露 depth | 外部误用导致参数级描述用错截断长度 |
| **P2** | D4 | 数组截断计数不精确 | 诊断信息偏高但不影响数据 |
| **P2** | D8 | 无广度限制 | 极端输入下 CPU 无保护 |
| **P3** | D2 | 无递归深度硬上限 | 极端嵌套 schema 有栈溢出理论风险 |
| **P3** | D5 | 大小写敏感的 drop_fields | PascalCase API 的调试字段漏过 |
| **P3** | D6 | Grapheme Cluster 截断 | 带重音/组合 emoji 的文本可能视觉断裂 |
| **P3** | D9 | 极端空数组只剩 marker | 边缘情况诊断信息不足 |
