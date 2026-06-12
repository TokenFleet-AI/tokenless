# Codex CLI Support Spec

> TDD: RED → GREEN → REFACTOR | 参考 RTK `hooks/codex/` 实现

## 需求

Tokenless 新增 OpenAI Codex CLI 支持。Codex 没有 hook 协议，采用 **Rules 文件**方式（Category D：Provider-Config 型）。

## 参考实现

RTK 项目 `/Users/byx/Documents/workspace/github.com/rtk-ai/rtk/`：
- `src/hooks/init.rs` — `run_codex_mode()`, `resolve_codex_dir()`, `patch_agents_md()`
- `hooks/codex/rtk-awareness.md` — 规则内容模板

## 改动文件

| 文件 | 改动 |
|------|------|
| `crates/tokenless-cli/src/init/mod.rs` | +Agent::Codex 枚举、+init_codex()、+辅助函数 |
| `crates/tokenless-cli/src/commands/init_cmd.rs` | +"codex" 映射分支 |

## 行为规格

### 项目级：`tokenless init --agent codex`

```
创建 ./RTK.md（写入 RTK_SLIM_CODEX 规则内容）
创建/修改 ./AGENTS.md（添加 @RTK.md 引用，保留已有内容）
```

### 全局：`tokenless init -g --agent codex`

```
解析 $CODEX_HOME → 回退 ~/.codex/
创建 $CODEX_HOME/RTK.md
创建/修改 $CODEX_HOME/AGENTS.md（使用绝对路径 @/abs/path/RTK.md）
```

**Issue #892 修复**：Codex 从 CWD 解析 `@` 引用，而非 AGENTS.md 所在目录。全局模式必须用绝对路径。

### 无须支持的特性

- `--compress / --no-compress / --passthrough / --debug` — Codex 无 hook，忽略
- `--uninstall` — 首版不包含，后续可加

## Agent 枚举新增

```rust
pub enum Agent {
    // ... 现有 12 个 ...
    Copilot,
    /// OpenAI Codex CLI (AGENTS.md + RTK.md rules, no hooks).
    Codex,
}
```

## 函数签名

```rust
/// 写入 RTK.md + AGENTS.md 引用
fn init_codex(config: &InitConfig) -> Result<(), String>;

/// 解析 Codex 配置目录：$CODEX_HOME → ~/.codex/
fn resolve_codex_dir() -> PathBuf;

/// 生成 Codex 规则内容（与 RTK 项目 hooks/codex/rtk-awareness.md 一致）
fn codex_rules() -> &'static str;

/// 在 AGENTS.md 中添加 @RTK.md 引用（幂等）
fn add_ref_to_agents_md(agents_md: &Path, rtk_ref: &str) -> Result<bool, String>;
```

## 测试清单

| # | 测试 | 验证点 |
|---|------|--------|
| 1 | `test_should_parse_agent_codex` | `"codex"` → `Agent::Codex` |
| 2 | `test_should_init_codex_writes_rtk_md` | 项目级写入 RTK.md |
| 3 | `test_should_init_codex_patches_agents_md` | AGENTS.md 添加 @RTK.md |
| 4 | `test_should_init_codex_global_uses_codex_home` | 全局写入 $CODEX_HOME |
| 5 | `test_should_init_codex_global_absolute_reference` | 绝对路径引用 |
| 6 | `test_should_init_codex_idempotent` | 重复 init 幂等 |
| 7 | `test_should_resolve_codex_dir_from_env` | $CODEX_HOME 优先 |
| 8 | `test_should_resolve_codex_dir_fallback` | 回退 ~/.codex/ |
| 9 | `test_should_init_codex_preserves_existing_agents_md` | 保留已有内容 |
| 10 | `test_should_init_codex_ignores_compress_flags` | compress 等 flag 不影响 |

## 实现步骤（TDD 顺序）

1. **RED** — 先写测试，`cargo test` 失败
2. **GREEN** — 实现 `Agent::Codex` + `init_codex()` + 辅助函数
3. **REFACTOR** — 消除重复（与现有 `write_file`、`merge_into_settings` 对齐）
4. 更新 `init_cmd.rs` 映射
5. `cargo test` 全绿 + `cargo clippy` 无警告

---

Owner: baoyx · 版本：v1.0 · 日期：2026-06-11
