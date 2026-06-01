# Security Hardening Specification

> 基于 rust-tui-template 安全审计 | 评估日期：2026-06-01 | 优先级：P0

## 背景

经由 RuFlo 6 角色多 Agent Swarm 评估，对比 `rust-tui-template` 模板的安全基线，tokenless 当前存在 9 项安全配置缺口。本文档定义补齐这些缺口的规格和实施计划。

参考：[模板迁移分析报告](../docs/research/template-migration-analysis.md)

---

## 威胁模型更新

### 新增威胁

| 威胁 | 当前状态 | 影响 |
|------|:---:|------|
| 供应链依赖替换 | `deny.toml` wildcards=allow | 恶意 crate 可在补丁版本注入 |
| 密钥意外提交 | `.gitignore` 无密钥规则 | `.env`/`.pem` 可被 commit |
| 已知漏洞依赖 | CI 无 `cargo audit` | 漏洞依赖可合并而不被发现 |
| 二进制内存利用 | 无链接器加固 | GOT 覆写、ROP 攻击面 |

### 信任边界（无变化）

参见 [0005-security-model-design.md](./0005-security-model-design.md)，本规格不改变现有信任边界，仅加固外部防御层。

---

## 安全加固项

### P0-1：`.gitignore` 密钥排除

**现状**：仅排除 `target/`, `.vscode/`, ruflo 运行时目录。无任何密钥/凭证排除规则。

**目标**：添加全面密钥排除模式。

**实施**：
```diff
 # Ruflo / Claude Code runtime
 .claude-flow/
 .swarm/
 .claude/
 .mcp.json
 ruvector.db
+
+# Environment & Secrets
+.env
+.env.*
+.env.local
+
+# Keys & Certificates
+*.pem
+*.key
+*.p12
+*.cer
+*.pfx
+
+# SSH
+.ssh/
+
+# Cloud credentials
+secrets/
+credentials/
+.aws/
+.gcloud/
```

**验证**：创建 `.env.test` 文件，`git status` 应不显示为 untracked。

---

### P0-2：gitleaks 预提交扫描

**现状**：`pre-commit-config.yaml` 中无 gitleaks 钩子。

**目标**：每次提交自动扫描硬编码密钥。

**实施**：
```yaml
- repo: https://github.com/gitleaks/gitleaks
  rev: v8.18.0
  hooks:
    - id: gitleaks
      args: ["protect", "--staged", "--verbose"]
      stages: [pre-commit]
```

**验证**：`pre-commit run gitleaks --all-files` 应通过。

---

### P0-3：强化 `deny.toml` 供应链策略

**现状**：
- `[bans]` wildcards = `"allow"` → 允许 `*` 版本约束
- `[sources]` unknown-registry = `"warn"`, unknown-git = `"warn"` → 不阻断

**目标**：与模板对齐，阻断不安全的依赖模式。

**实施**：
```toml
[bans]
wildcards = "deny"
# 新增：禁止 openssl-sys（优先使用 rustls）
[[bans.deny]]
name = "openssl-sys"

[sources]
unknown-registry = "deny"
unknown-git = "deny"

# 新增：禁止 reqwest 的 native-tls feature
[[bans.features]]
name = "reqwest"
deny = ["native-tls", "native-tls-alpn", "native-tls-vendored"]
```

**验证**：`cargo deny check bans` 和 `cargo deny check sources` 应通过。

---

### P0-4：CI 增加 `cargo audit`

**现状**：`cargo audit` 仅在 Makefile 中可通过 `make audit` 手动运行，不在 CI 中。

**目标**：每次 PR 自动扫描已知漏洞。

**实施**：在 CI workflow 中添加：
```yaml
- name: Security audit
  run: cargo audit
```

**验证**：CI 运行应包含 audit 步骤。

---

### P0-5：CI 最小权限 + 凭证保护

**现状**：
- 未设置顶层 `permissions` 块（默认写权限）
- `actions/checkout` 未设置 `persist-credentials: false`

**目标**：遵循最小权限原则。

**实施**：
```yaml
permissions:
  contents: read

jobs:
  ci:
    steps:
      - uses: actions/checkout@v4
        with:
          persist-credentials: false
```

对 `release.yml` 中的发布 job 单独设置 `contents: write`。

**验证**：CI 运行通过。

---

### P1-1：链接器安全加固

**现状**：无 `.cargo/config.toml`，二进制缺少平台安全标志。

**目标**：全平台链接器加固。

**实施**：创建 `.cargo/config.toml`：
```toml
[target.x86_64-unknown-linux-gnu]
rustflags = ["-C", "link-args=-Wl,-z,relro,-z,now"]

[target.aarch64-unknown-linux-gnu]
rustflags = ["-C", "link-args=-Wl,-z,relro,-z,now"]

[target.x86_64-apple-darwin]
rustflags = ["-C", "link-args=-Wl,-dead_strip"]

[target.aarch64-apple-darwin]
rustflags = ["-C", "link-args=-Wl,-dead_strip"]

[target.x86_64-pc-windows-msvc]
rustflags = ["-C", "link-args=/DYNAMICBASE /NXCOMPAT /HIGHENTROPYVA"]

[alias]
check-all = "check --workspace --all-targets --all-features"
```

**验证**：`readelf -l target/release/tokenless | grep -E 'RELRO|BIND_NOW'`（Linux）

---

### P1-2：crate-level `#![forbid(unsafe_code)]`

**现状**：仅 `tokenless-tui` 有 crate-level forbid。`tokenless-schema` 和 `tokenless-stats` 缺失。

**目标**：所有 crate 统一禁止 unsafe。

**实施**：在 `tokenless-schema/src/lib.rs` 和 `tokenless-stats/src/lib.rs` 顶部添加：
```rust
#![forbid(unsafe_code)]
```

**验证**：`cargo check --workspace` 通过。

---

### P2-1：构建来源证明

**现状**：发布二进制无来源证明。

**目标**：SLSA 构建来源可溯源。

**实施**：在 `release.yml` 中添加：
```yaml
- name: Attest build provenance
  uses: actions/attest-build-provenance@v2
  with:
    subject-path: "artifacts/*"
```

---

## 实施检查清单

| # | 项目 | 优先级 | 阶段 | 状态 |
|:---:|------|:---:|:---:|:---:|
| 1 | `.gitignore` 密钥排除 | P0 | 阶段一 | ✅ |
| 2 | gitleaks 预提交 | P0 | 阶段一 | ✅ |
| 3 | `deny.toml` wildcards=deny | P0 | 阶段一 | ✅ |
| 4 | `deny.toml` sources=deny | P0 | 阶段一 | ✅ |
| 5 | `deny.toml` 禁止 openssl-sys | P0 | 阶段一 | ✅ |
| 6 | `deny.toml` 禁止 native-tls | P0 | 阶段一 | ✅ |
| 7 | CI `cargo audit` | P0 | 阶段二 | ✅ |
| 8 | CI `cargo deny check` | P0 | 阶段二 | ✅ |
| 9 | CI `permissions: contents: read` | P0 | 阶段二 | ✅ |
| 10 | CI `persist-credentials: false` | P0 | 阶段二 | ✅ |
| 11 | `.cargo/config.toml` 链接器加固 | P1 | 阶段一 | ✅ |
| 12 | crate-level `#![forbid(unsafe_code)]` | P1 | 阶段四 | ✅ |
| 13 | 构建来源证明 | P2 | 阶段二 | ✅ |
| 14 | `secrecy` crate 集成 | P2 | 阶段五 | ⬜ |

---

## 验证流程

每完成一个阶段后执行：

```bash
# 阶段一验证
pre-commit run --all-files
cargo deny check bans
cargo deny check sources
git status  # 确认无密钥文件暴露

# 阶段二验证
# 检查 CI workflow 运行日志包含 audit/deny 步骤
cargo audit
cargo deny check

# 阶段一+二综合验证
make lint
make audit
```

---

## 相关文档

- [0005-security-model-design.md](./0005-security-model-design.md) — 安全模型设计
- [Template Migration Analysis](../docs/research/template-migration-analysis.md) — 模板迁移分析
- [0016-architecture-alignment.md](./0016-architecture-alignment.md) — 架构对齐规格
