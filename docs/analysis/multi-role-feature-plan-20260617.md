# Tokenless 多角色并行分析 — 综合功能规划（2026-06-17）

> 6 个专业 Agent 并行深度分析，130KB 报告汇总。
> 本报告提炼跨维度共识，形成功能规划建议。

## 一、各角色评分总览

| 维度 | Agent | 评分 | 一句话结论 |
|------|-------|------|-----------|
| 架构 | `architecture-analyst` | **7.5/10** | 分层清晰但 spec 失配，hook.rs 849 行巨石 + 扩展点缺 trait |
| 安全 | `security-analyst` | **7.5/10** | 基线高但 `sh -c` 注入面 + MCP 参数无上限 + 路径未 canonicalize |
| 性能 | `performance-analyst` | **7/10** | 双序列化零收益保护 + Vec 线性 LRU + 无 bench 基线 |
| UX/DevRel | `ux-devrel-analyst` | **7.2/10** | 安装路径长 + 文档不一致 + 缺 `doctor`/`status`/`share` |
| 市场 | `market-innovation-analyst` | — | 定位为 Agent Token Middleware，优先团队成本控制台 |
| 代码质量 | `code-quality-analyst` | **7/10** | CLI/TUI 测试薄弱 + pedantic 未过 + pub 文档覆盖 2.3%~50% |

**综合健康度**：**7.2/10** — 已脱离原型期，进入"可维护但尚未 fully hardened"阶段。

---

## 二、跨维度高频共识（3+ Agent 同时推荐）

### 🔴 P0 — 立即执行（本月内启动）

| # | 功能/改进 | 推荐 Agent | 复杂度 | 预期影响 |
|---|---------|-----------|--------|---------|
| 1 | **Criterion 基准测试基线** | 性能、代码质量 | 中 | 🔥 所有后续优化的前提 |
| 2 | **清零 clippy pedantic 债务** | 代码质量、架构 | 中 | 🔥 质量门闭环 |
| 3 | **`tokenless doctor` 新手诊断** | UX、市场 | 中 | 🔥 降低安装失败率 |
| 4 | **`tokenless status` 轻量状态页** | UX、市场 | 低-中 | 🔥 日常留存入口 |
| 5 | **Spec 0017 实施（stats 历史管理）** | 市场、UX、代码质量 | 低 | 🔥 用户感知价值最直接 |
| 6 | **Hook Adapter 插件化（拆 hook.rs 849 行）** | 架构、安全 | 高 | 🔥 最大架构债务 |
| 7 | **dirty flag 替代双序列化零收益保护** | 性能 | 中 | 🔥 消除热路径最大热点 |
| 8 | **PredictCache 重写为 O(1) LRU** | 性能 | 中 | 🔥 解除全局 Mutex 瓶颈 |

### 🟡 P1 — 近期执行（1-3 个月内）

| # | 功能/改进 | 推荐 Agent | 复杂度 | 预期影响 |
|---|---------|-----------|--------|---------|
| 9 | **Spec 0018 实施（压缩统计回传）** | 市场、架构 | 中 | 商业化底座 |
| 10 | **策略注册表 + 评分式路由器** | 架构 | 高 | 可扩展性核心 |
| 11 | **统一输入校验层 `ValidatedMcpArgs`** | 安全 | 中 | 边界即拒绝 |
| 12 | **移除所有 `sh -c` 依赖检查** | 安全 | 低-中 | 消除 shell 注入面 |
| 13 | **路径安全加固（canonicalize + symlink check）** | 安全 | 中 | 防本地攻击 |
| 14 | **编码器改 writer API（减少中间分配）** | 性能 | 中 | 减少 allocation 风暴 |
| 15 | **CLI 端到端测试层（assert_cmd + insta）** | 代码质量 | 中 | 真实 E2E 覆盖 |
| 16 | **pub API rustdoc 补齐** | 代码质量、UX | 中 | 文档覆盖率 |
| 17 | **`tokenless stats share` 分享型报告** | UX、市场 | 中 | 传播裂变 |
| 18 | **统一压缩 explain/report 观测层** | 架构、UX | 中 | 可解释性 |
| 19 | **Spec 0014 Level 1（规则级语义压缩）** | 市场、架构 | 高 | 护城河方向 |
| 20 | **统一文档口径（README/user-guide/spec 对齐）** | UX | 低 | 消除用户困惑 |

### 🟢 P2 — 中期规划（3-12 个月）

| # | 功能/改进 | 推荐 Agent | 复杂度 | 预期影响 |
|---|---------|-----------|--------|---------|
| 21 | **敏感文本治理升级（secrecy + redaction engine）** | 安全 | 高 | 硬化日志安全 |
| 22 | **`tokenless --secure-default` 安全模式** | 安全 | 低-中 | 一键加固 |
| 23 | **stats 展示与存储解耦** | 架构 | 中 | 多展示层支持 |
| 24 | **异步远程语义 provider 抽象** | 架构 | 高 | 未来扩展 |
| 25 | **WASM Build（`@tokenfleet/tokenless-wasm`）** | 市场 | 高 | 新分发渠道 |
| 26 | **团队成本控制台（Team Savings Dashboard）** | 市场 | 高 | 商业化核心 |
| 27 | **策略配置中心** | 市场、架构 | 高 | 企业级治理 |
| 28 | **Agent 能力矩阵页 + 场景化安装向导** | UX | 低 | 降低决策成本 |
| 29 | **覆盖率基线 + PR 门禁（cargo llvm-cov）** | 代码质量 | 中 | 持续质量保障 |
| 30 | **docs/examples recipes 库** | UX | 低 | 场景化学习 |

---

## 三、建议路线图

### 第一阶段：基线建立 + 速赢（1-4 周）

**目标**：建立可量化基线，修复最明显的用户/工程痛点。

```
Week 1-2:
  ├── #1 Criterion benchmark 基线（schema + cli + cache）
  ├── #7 dirty flag 替代双序列化
  ├── #8 PredictCache → O(1) LRU + hit/miss 遥测
  ├── #4 tokenless status（低复杂度速赢）
  └── #20 统一文档口径

Week 3-4:
  ├── #2 清零 clippy pedantic 债务
  ├── #3 tokenless doctor
  ├── #5 Spec 0017 实施（stats delete/vacuum/export）
  └── #12 移除 sh -c 依赖检查
```

**交付物**：bench 数据、2 个新命令、pedantic 绿灯、0017 落地。

### 第二阶段：架构加固 + 安全硬化（4-8 周）

**目标**：解决最大架构债务，达到安全硬化水平。

```
Week 5-6:
  ├── #6 Hook Adapter 插件化（拆 hook.rs）
  ├── #10 策略注册表 + trait
  ├── #11 统一输入校验层
  └── #13 路径安全加固

Week 7-8:
  ├── #9 Spec 0018 实施
  ├── #14 编码器 writer API
  ├── #15 CLI E2E 测试层
  └── #17 stats share 分享报告
```

**交付物**：hook adapter trait、MCP 输入校验、stats 回传协议、分享功能。

### 第三阶段：创新 + 商业化（8-24 周）

**目标**：建立护城河，启动商业化探索。

```
Week 9-16:
  ├── #19 Spec 0014 Level 1（规则级语义压缩）
  ├── #18 explain/report 观测层
  ├── #16 pub API rustdoc 补齐
  └── #25 WASM Build 探索

Week 17-24:
  ├── #26 团队成本控制台 MVP
  ├── #27 策略配置中心
  └── #24 异步远程语义 provider
```

---

## 四、风险与缓解

| 风险 | 影响 | 缓解 |
|------|------|------|
| pedantic 修复引入行为变化 | 回归 | 每个修复配 golden snapshot 验证 |
| hook.rs 拆分影响 11 Agent 协议 | 功能回归 | 先建 E2E 测试层再重构 |
| WASM 化需替代 rusqlite | 范围膨胀 | 先 tokenless-schema 单 crate WASM 化 |
| 团队控制台需要 auth/多租户 | 复杂度爆炸 | 先做本地 JSON 导出，后做 SaaS |
| Criterion 基线暴露严重性能问题 | 需要大量修复 | 设阈值，只修 >20% 偏离项 |

---

## 五、各角色详细报告索引

| 报告 | 路径 | 大小 |
|------|------|------|
| 架构评审 | `docs/analysis/architecture-review-20260617.md` | 21KB |
| 安全评审 | `docs/analysis/security-review-20260617.md` | 23KB |
| 性能评审 | `docs/analysis/performance-review-20260617.md` | 20KB |
| UX/DevRel 评审 | `docs/analysis/ux-devrel-review-20260617.md` | 22KB |
| 市场/创新评审 | `docs/analysis/market-innovation-review-20260617.md` | 18KB |
| 代码质量评审 | `docs/analysis/code-quality-review-20260617.md` | 26KB |
| **综合功能规划** | `docs/analysis/multi-role-feature-plan-20260617.md` | 本文件 |

---

## 六、建议的立即行动项

如果只选 **3 件事**立即开始：

1. **建立 Criterion 基准测试**（性能 Agent 反复强调：没有数据，一切优化都是猜测）
2. **实施 Spec 0017**（stats 历史管理 — 最快用户感知价值，开发成本仅 ~50 行）
3. **新增 `tokenless doctor` + `tokenless status`**（UX 速赢，显著降低新用户上手门槛）

这三件事覆盖了 **性能基线 + 用户价值 + 增长飞轮** 三个维度，且互不依赖，可并行推进。
