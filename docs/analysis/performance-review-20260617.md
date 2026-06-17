# Tokenless 性能评审报告（2026-06-17）

## 总体性能评分：7/10

**评分理由**
- 优点：核心压缩逻辑已经保持纯函数风格，具备可缓存性；对描述截断、结构遍历、格式路由都做了较明确的职责拆分；已有输入大小上限与部分高保真/标准压缩配置，整体架构适合继续优化。
- 扣分点：热路径上仍存在多处“解析一次、压缩一次、再序列化两次”的重复工作；缓存实现是 `Vec` 线性 LRU + 全局 `Mutex`，在 Hook 高频场景下会成为明显串行瓶颈；格式路由与编码器中存在较多 `String` 拼接和中间 `Vec<String>` 分配；目前缺少任何 Criterion bench，导致优化优先级主要依赖静态阅读而非数据驱动。
- 综合判断：当前实现对于中小输入可用，但作为嵌入 Agent Hook 的延迟敏感链路，距离“高频、并发、可量化优化”的成熟状态还有一段距离。

## 一、热路径分析结果

### 1.1 端到端热路径
按调用频率与累计成本看，典型路径如下：

1. CLI/MCP 读取输入
2. `serde_json::from_str()` 解析 JSON
3. 选择压缩器或格式路由
4. 深度遍历 `serde_json::Value`
5. `serde_json::to_string()` 生成紧凑输出用于 token 估算
6. `serde_json::to_string_pretty()` 生成最终输出
7. 可选：进入预测缓存（命中则跳过中间步骤）

对应关键位置：
- `/Users/byx/Documents/workspace/github.com/TokenFleet-AI/tokenless/crates/tokenless-cli/src/commands/compress.rs:25-70`
- `/Users/byx/Documents/workspace/github.com/TokenFleet-AI/tokenless/crates/tokenless-cli/src/commands/compress.rs:104-175`
- `/Users/byx/Documents/workspace/github.com/TokenFleet-AI/tokenless/crates/tokenless-cli/src/commands/compress.rs:216-247`
- `/Users/byx/Documents/workspace/github.com/TokenFleet-AI/tokenless/crates/tokenless-schema/src/schema_compressor.rs:169-255`
- `/Users/byx/Documents/workspace/github.com/TokenFleet-AI/tokenless/crates/tokenless-schema/src/response_compressor.rs:183-191`
- `/Users/byx/Documents/workspace/github.com/TokenFleet-AI/tokenless/crates/tokenless-schema/src/format_router.rs:97-119`

### 1.2 最值得关注的热点

#### 热点 A：零收益保护带来的双序列化
`SchemaCompressor::compress()` 和 `ResponseCompressor::compress()` 都采用“压缩前 `to_string` + 压缩后 `to_string`”比较文本是否相同，再决定返回原值还是结果。

- `schema_compressor.rs:169-255`
- `response_compressor.rs:183-191`

这意味着每次压缩至少多出两次完整 JSON 序列化。对于 100KB 级输入，这部分非常容易逼近甚至超过实际压缩遍历本身的成本。

#### 热点 B：深拷贝 `Value`
`SchemaCompressor::compress()` 一开始就执行 `let mut result = tool.clone();`，在无收益时还会 `return tool.clone();`。

- `schema_compressor.rs:174`
- `schema_compressor.rs:251-253`

这会让大 schema 在“轻度压缩”或“无变化”场景下付出一次甚至两次深拷贝代价。

#### 热点 C：CLI 层再次序列化
CLI 在压缩后先生成紧凑 JSON 用于 token 估算，再生成 pretty JSON 用于输出：
- `compress.rs:34-50`
- `compress.rs:151-155`
- `compress.rs:223-226`

这是合理的产品行为，但当前它与压缩器内部的“零收益保护序列化”叠加，形成 3-4 次序列化链路。

#### 热点 D：格式路由额外完整遍历
`compress_auto_with()` 在真正编码前先运行 `shape_analyzer::analyze()` 做一次 O(n) 遍历，然后编码器本身再遍历一次。

- `format_router.rs:111-118`
- `shape_analyzer.rs:48-83`

对大输入来说，自动路由模式本质是“分析一次 + 编码一次 + CLI 再序列化一次”。如果命中率不高，自动路由的附加成本需要被 bench 验证是否值得。

### 1.3 主要 allocation / clone 观察

#### Schema 压缩
- 深 clone `Value`：`schema_compressor.rs:174`
- `truncate_description()` 中多次 `into_owned()`，会为 markdown 移除、空白压缩生成新字符串：`schema_compressor.rs:445-505`
- `compress_json_schema()` 在处理 description 时 `map(String::from)` 会复制原描述：`schema_compressor.rs:277-295`

#### Response 压缩
- 默认构造会分配两组 `HashSet<String>`：`response_compressor.rs:33-71`
- `compress_object()` 对保留字段直接 `value.clone()`，对普通字段递归后再插入，整体是“新树构建”模型：`response_compressor.rs:272-314`
- 数组/对象路径广泛使用 `Vec<String>` + `join()`，中间字符串较多：`response_compressor.rs:238-270`

#### 编码器
- `toon_hrv.rs:49-57` 为 header 与全部 rows 构造 `Vec<String>` 再 `join`
- `toon_hrv.rs:78-86` 每行构建 `Vec<String>` cell 列表
- `toon_hrv.rs:110-119` 嵌套数组/对象递归转成临时 `Vec<String>`
- `enhanced_toon.rs:54-97`、`101-110`、`207-295` 同样大量依赖 `Vec<String>`、`join`、`format!`
- `cjson_compact.rs:56-66` 为数组/对象每层都构造 `Vec<String>`

结论：当前实现更多是“代码清晰优先”的字符串拼接风格，不是“最少分配优先”的 writer 风格。

## 二、逐项分析

### 2.1 热路径分析：是否有不必要的 clone / allocation？

**发现**
1. `SchemaCompressor` 存在固定深拷贝，并在无收益时可能再次 clone。
2. `SchemaCompressor`/`ResponseCompressor` 的零收益保护依赖双序列化比较，属于高频额外成本。
3. `ResponseCompressor::default()` 每次构造都会分配 `drop_fields` 与 `preserve_fields` 两个 `HashSet<String>`；CLI 已经通过 `LazyLock` 复用静态实例缓解了主路径问题，但 MCP 动态参数路径仍会重新构造。
4. 三种编码器大量使用 `Vec<String>` + `join()`，适合小输入，但对大数组、大对象会产生明显的短生命周期分配风暴。
5. `shape_analyzer::check_uniform_array()` 会对首项及最多前 100 项的 key 做排序比较：`shape_analyzer.rs:143-175`。对于字段多的对象数组，这部分是可见成本。

**判断**
- 中小输入下主要瓶颈是序列化/分配次数过多。
- 大输入下主要瓶颈会转向：深 clone + 多轮完整遍历 + 字符串聚合。

### 2.2 缓存效率：LRU 命中率、blake3 成本、策略是否最优？

关键实现：
- `/Users/byx/Documents/workspace/github.com/TokenFleet-AI/tokenless/crates/tokenless-cli/src/cache.rs:23-89`
- `/Users/byx/Documents/workspace/github.com/TokenFleet-AI/tokenless/crates/tokenless-cli/src/cache.rs:91-126`

**发现**
1. 当前 LRU 结构是 `Vec<(u64, String)>`。
   - 查找：`iter().position(...)`，O(n)
   - 命中提升到 MRU：`remove(pos)` + `insert(0, entry)`，也是 O(n)
   - 插入淘汰同样为 O(n)
2. 容量默认 512。对单用户 CLI 场景尚可，但对 Hook 高频、输入模式多样的会话来说，512 并不大，而且线性 LRU 在命中时也要搬移元素。
3. 全局缓存用 `LazyLock<Mutex<PredictCache>>` 包裹，意味着所有读写完全串行：`cache.rs:93-126`。
4. key 生成使用 `blake3` 对完整输入哈希，且会先 `format!("v{}:{}", ...)` 生成一个 versioned 字符串：`cache.rs:43-47`。这引入一次额外分配。
5. value 保存的是完整 `String` 输出；若缓存的是 pretty 输出，大对象会显著放大内存占用。

**对命中率的判断**
- 当前代码没有任何 hit/miss/evict 指标，因此无法真实回答“命中率如何”，这本身就是一个监控缺口。
- 从使用点看，缓存只按原始输入字符串键控：
  - `compress.rs:26,69,105,174,217,246`
  - `toon.rs:19,27`
- 对“同一工具 schema 重复发送”“同一命令轮询”“同一响应重复压缩”的场景，命中率可能很高。
- 对包含时间戳、随机字段、日志尾巴的输出，命中率会非常低，哈希成本与锁竞争反而是纯额外开销。

**blake3 开销是否合理**
- 结论：算法本身合理，问题不在 blake3，而在“每次都要哈希整段输入 + 额外分配 versioned 字符串 + 串行锁 + O(n) LRU”。
- 对 1KB 输入，hash 成本通常可接受。
- 对 100KB+ 输入，hash 仍然不算太贵，但若缓存命中率低，则 hash 不再划算。

**缓存策略是否最优**
- 不是。
- 当前更像“简单可用版本”，并非适合 Hook 高频场景的最终策略。
- 更优方向应为：`HashMap + intrusive/linked LRU` 或现成 `lru` crate；按操作类型分段缓存；增加命中率遥测；对大输入做阈值策略。

### 2.3 序列化开销：`serde_json` parse/serialize 是否是瓶颈？是否可零拷贝？

**发现**
1. CLI 入口普遍先 `serde_json::from_str()`，这是不可避免的一次完整 parse：`compress.rs:30-31`, `109-110`, `221-222`
2. 压缩器内部又做两次 `to_string()` 用于零收益比较。
3. CLI 又做紧凑输出和 pretty 输出两次序列化。
4. `compress_auto()` 返回 `String` 编码结果，但 CLI 仍把该字符串再次 `serde_json::to_string(&result)`，本质是在给文本再包一层 JSON 字符串：`compress.rs:223-226`。这里的 token 估算与最终展示语义并不完全一致，且会引入一次额外转义序列化。

**判断**
- 对 schema/response 压缩，`serde_json` parse/serialize 很可能已经是头部瓶颈之一。
- 对自动编码路径，这个结论更强，因为路由分析与编码都不是零成本。

**是否可以零拷贝**
- 完全零拷贝很难，因为当前逻辑基于 `serde_json::Value` 做变换与重组。
- 但“减少复制”是完全可行的：
  1. 用 dirty flag 替代“前后序列化比较”。
  2. 对输出统一走一次 `to_writer` 写入 `String`/`Vec<u8>`，减少中间 `String`。
  3. 对纯编码器改为 writer API，例如 `fn encode_into(value: &Value, out: &mut String)`。
  4. 对仅做 shape 判断的路径，可考虑轻量扫描而非完整二次遍历，但这需要以 bench 证明收益。

### 2.4 并发模型：当前单线程是否足够？是否需要并行？

关键共享点：
- `PredictCache`: `cache.rs:93`
- diff 基线缓存：`cache.rs:132-179`
- semantic compressor: `shared.rs:17-18`
- stats recorder 内部 SQLite `Mutex`（由 spec 与 shared 调用链可见）

**发现**
1. 当前共享状态几乎都靠全局 `Mutex` 串行化。
2. CLI 单次执行通常问题不大，但 Hook 场景下可能是多进程/多调用并发，而 MCP server 模式则可能是单进程内多请求并发。
3. 即使 Rust 代码本身没有主动并行，调用方仍可能在短时间内发起多次工具调用。
4. `PredictCache` 与 diff cache 都是全局锁，且临界区内会执行 O(n) 查找/搬移。

**是否需要并行**
- 对“单次大 JSON 压缩”而言，不建议立即引入内部并行遍历，收益未必覆盖调度和分片开销。
- 对“多请求并发”而言，需要至少做到共享热点不成为锁瓶颈，这比内部并行更优先。

**建议判断**
- 近期不建议把 schema/response 压缩递归逻辑改成 Rayon 并行。
- 更优先的是：
  1. 降低全局锁粒度；
  2. 将缓存改为更适合并发的实现；
  3. 为 MCP server 基准验证多并发 QPS 与 p95/p99。

### 2.5 大输入处理：100KB+ JSON 是否有问题？是否需要流式处理？

**现状**
- `read_input()` 对输入设置了 10MB 上限：`shared.rs:64-99`
- 这避免了灾难性 OOM，但不代表 100KB+ 就高效。

**风险点**
1. 100KB+ 时，多轮 `to_string()`、深 clone、pretty-print 都会明显抬高延迟。
2. `SchemaCompressor` / `ResponseCompressor` 都是整树驻留内存模型，不是流式。
3. 编码器也都是构建完整 `String` 输出，无法边遍历边写流。
4. 大对象数组走 TOON HRV 时，会先为每行与每个 cell 生成中间 `String`，峰值分配较高。
5. 自动路由模式下，`shape_analyzer` 先遍历一次，编码器再遍历一次，100KB+ 时非常容易出现“分析成本抵消压缩收益”的情况，尤其在输出本就不适合编码时。

**是否需要流式处理**
- 对当前产品目标，全面引入流式 JSON 变换复杂度较高，不应作为第一优先级。
- 但对 100KB-1MB 区间，建议至少实现“流式输出 writer 化”，即遍历仍基于 `Value`，输出不再层层拼接 `String`。
- 对超大响应压缩，如果未来要处理 MB 级日志或大数组，再考虑基于 `serde_json::Deserializer` 的流式扫描/截断版本。

### 2.6 基准测试缺失：0 个 bench 文件的影响

**影响**
1. 无法区分“理论热点”和“真实热点”。
2. 无法评估自动路由相比纯 ResponseCompressor 的收益/成本比。
3. 无法量化 512 容量缓存是否合适，也无法判断 hash 与锁的开销是否值得。
4. 优化后容易引入回归却没有基线。

**当前最缺的不是更多单元测试，而是稳定的性能基线。**

### 2.7 新功能规划：推荐的性能改进与新能力

下面给出 5 项建议，兼顾性能改进与可交付新特性。

## 三、具体性能改进建议

| 建议 | 优先级 | 复杂度 | 预期收益 | 说明 |
|---|---|---:|---:|---|
| 用 dirty flag / 变更计数替代双序列化零收益保护 | P0 | 中 | 高 | 去掉压缩器内部两次 `to_string()`，对大输入收益最大 |
| 重写 PredictCache 为 O(1) LRU，并补齐命中率遥测 | P0 | 中 | 高 | 降低锁内线性扫描与元素搬移，支撑 Hook 高频场景 |
| 将编码器改为 `write!`/`push_str` writer 风格 | P1 | 中 | 中-高 | 显著减少 `Vec<String>` 与 `join()` 带来的中间分配 |
| 增加大小阈值与自适应缓存策略 | P1 | 低-中 | 中 | 对低命中大输入绕过缓存或只缓存 schema 类稳定输入 |
| 增加性能 profile/遥测输出 | P1 | 中 | 中 | 为后续优化提供 hit rate、p95、平均输入大小等基础数据 |

### 3.1 建议一：去掉压缩器内部双序列化比较

**问题位置**
- `schema_compressor.rs:170-173`, `247-253`
- `response_compressor.rs:184-189`

**方案**
- 在遍历过程中维护 `changed: bool`。
- 真正发生字段删除、截断、值替换时置位。
- `compress()` 最后根据 `changed` 决定返回原始值还是结果。

**预期收益**
- 对 10KB-100KB 输入，通常是单项最高收益优化。
- 预计可减少一次压缩调用中的 20%-40% 序列化相关开销，具体需 bench 验证。

### 3.2 建议二：将 PredictCache 升级为 O(1) LRU + 指标

**问题位置**
- `cache.rs:23-89`

**方案**
- 使用 `lru` crate 或 `HashMap + linked list`。
- 统计 `hit/miss/evict/bytes`。
- 将 `hash_key()` 改为避免 `format!`：连续 update version bytes 与 input bytes 即可。
- 视情况把 value 改为 `Arc<str>`，减少 clone 成本。

**预期收益**
- 高频命中场景可显著降低锁持有时间与缓存管理成本。
- 为缓存是否继续保留提供数据依据。

### 3.3 建议三：编码器改成 writer API

**问题位置**
- `toon_hrv.rs:48-57`, `78-119`
- `enhanced_toon.rs:54-110`, `207-295`
- `cjson_compact.rs:56-66`

**方案**
- 从 `fn encode(...) -> String` 演进为内部 `fn encode_into(..., out: &mut String)`。
- 使用 `out.push_str()`、`write!()` 逐步构造结果。
- 对数组、对象避免 `Vec<String>` + `join()`。

**预期收益**
- 对大数组和深层对象，能明显减少临时分配与峰值内存。
- TOON HRV 会是收益最大的编码器。

### 3.4 建议四：缓存策略做输入分类

**新功能方向**
- 新增“自适应缓存”：
  - schema 输入默认缓存
  - 重复命令 rewrite 缓存
  - 动态响应（带时间戳/日志）的压缩默认低优先级缓存或跳过缓存
- 可基于大小阈值：例如 `>64KB` 且近 N 次命中率低时绕过缓存

**预期收益**
- 避免“低命中大对象”把 hash+锁+内存都浪费掉。

### 3.5 建议五：性能 profile / telemetry 模式

**新功能方向**
新增性能剖析输出，例如：
- `tokenless perf --json`
- 输出 parse/compress/encode/cache/serialize 各阶段耗时
- 输出 cache hit/miss、平均输入大小、p95/p99

**预期收益**
- 这是后续所有优化的观测基础。
- 也有助于用户根据实际 workload 调整 `TOKENLESS_CACHE_SIZE`、是否启用 experimental 等配置。

## 四、基准测试计划

## 4.1 应新增的 benchmark 维度

建议在 `crates/tokenless-schema/benches/` 与 `crates/tokenless-cli/benches/` 下新增 Criterion 基准。

### A. Schema 压缩基准
覆盖：
1. 小 schema（2KB）
2. 中 schema（20KB）
3. 大 schema（100KB）
4. 极大 enum schema（大量 `enum`）
5. 深层嵌套 schema

对比指标：
- 原始 `SchemaCompressor::compress()`
- 去除双序列化后的版本
- 是否 batch 模式

### B. Response 压缩基准
覆盖：
1. API 常规 JSON（5KB）
2. 大对象数组（100 条 / 1000 条）
3. 日志型响应（长字符串）
4. 高保真模式 vs 标准模式
5. 100KB / 500KB 输入

对比指标：
- 吞吐量（MB/s）
- 分配次数（可结合 heaptrack/dhat）
- 压缩收益与耗时比

### C. 格式路由与编码器基准
覆盖：
1. `shape_analyzer::analyze()` 单独基准
2. `encode_toon_hrv()`：均匀数组 10 / 100 / 1000 项
3. `encode_enhanced()`：schema-like 深层对象
4. `encode_cjson()`：不规则混合对象
5. `compress_auto_with()` 端到端与“直接 compressor”对比

重点回答：
- 自动路由是否真的比固定策略更划算？
- 100KB+ 下分析成本是否过高？

### D. 缓存基准
覆盖：
1. `cache_get` hit/miss
2. 64 / 512 / 4096 容量对比
3. 热 key 场景
4. 全 miss 场景
5. 4/8/16 线程并发下全局 `Mutex` 的影响

重点回答：
- 当前 `Vec` LRU 在 512 容量时还能否接受？
- 在并发模式下锁竞争有多严重？

### E. CLI 端到端基准
覆盖：
1. `compress-schema`
2. `compress-response`
3. `compress-auto`
4. 缓存冷启动 vs 热命中
5. pretty 输出开启下的端到端耗时

## 4.2 建议的 benchmark 样例矩阵

| Bench 名称 | 输入规模 | 目标 |
|---|---:|---|
| schema_small | 2KB | 基础回归 |
| schema_large_100kb | 100KB | 评估 clone + 双序列化成本 |
| response_logs_100kb | 100KB | 评估长字符串截断与 pretty 输出成本 |
| auto_uniform_array_1000 | 1000 项 | 测 HRV 路由与编码开销 |
| auto_irregular_100kb | 100KB | 测 analyze + cjson 的总成本 |
| cache_hit_hot_512 | 热命中 | 测 hit path |
| cache_miss_random_512 | 冷 miss | 测 hash + lock + insert 的纯成本 |
| cache_parallel_8threads | 并发 | 测 Mutex 串行化影响 |

## 五、性能优化路线图

### Phase 1：建立基线（1 周）
1. 引入 Criterion bench
2. 补充缓存 hit/miss/evict 遥测
3. 为 CLI 端到端增加 parse/compress/serialize 分阶段计时

**目标**：先确认真实热点排名，而不是凭直觉改写。

### Phase 2：低风险高收益优化（1-2 周）
1. 用 dirty flag 替代双序列化零收益保护
2. 去除 `hash_key()` 里的 `format!` 分配
3. MCP/动态路径尽量复用静态压缩器或共享默认集合
4. 优化 `compress-auto` 中对字符串结果再次 JSON 序列化的逻辑

**目标**：把单次调用平均延迟先降一轮。

### Phase 3：缓存与编码器重构（2 周）
1. PredictCache 升级为 O(1) LRU
2. 编码器改为 writer 风格
3. 增加自适应缓存策略

**目标**：降低大输入和高频场景的分配与锁竞争。

### Phase 4：并发与大输入专项（2 周）
1. 对 MCP server 进行并发压测
2. 评估分片缓存或读写锁/无锁方案
3. 视数据决定是否引入流式输出或大输入专用路径

**目标**：确保 Hook 嵌入场景下 p95/p99 可控。

## 六、结论

Tokenless 当前的性能问题不是“算法完全错误”，而是典型的“第一版实现已经足够正确，但还没完成热路径收敛”。

最核心的三个结论是：
1. **当前最大热点不是某一个复杂算法，而是重复序列化、深 clone 与字符串中间分配。**
2. **当前缓存的主要问题不是 blake3，而是 `Vec` 线性 LRU + 全局 `Mutex` + 缺少命中率数据。**
3. **当前最紧急的缺口不是继续猜测，而是尽快建立 bench 与遥测，让优化从静态分析转为数据驱动。**

如果只做三件事，我建议优先顺序是：
1. 加 bench 与性能遥测；
2. 去掉压缩器内部双序列化；
3. 重写 PredictCache。

这三项完成后，项目的总体性能评分有机会从 **7/10** 提升到 **8.5/10**。
