# Tokenless 发布与分发方案

## 一、分发渠道总览

| 渠道 | 内容 | 用户安装方式 | 受众 |
|------|------|-------------|------|
| crates.io | tokenless-schema / tokenless-stats / tokenless-cli | `cargo install tokenless` | Rust 开发者 |
| npm | @tokenless/openclaw-plugin | `openclaw plugins install @tokenless/openclaw-plugin` | OpenClaw 用户 |
| GitHub Release | tokenless binary (各平台) | 下载 + 解压 | 非 Rust 用户 |
| GitHub 源码 | adapters | `git clone + make` | 开发者 / 自建 |
| Homebrew | tokenless formula | `brew install TokenFleet-AI/tap/tokenless` | macOS 用户 |

---

## 二、crates.io 发布（核心库 + CLI）

### 需要发布的 crate

| Crate | 名称 | 说明 |
|-------|------|------|
| `tokenless-schema` | `tokenless-schema` | SchemaCompressor + ResponseCompressor |
| `tokenless-semantic` | `tokenless-semantic` | 语义向量 / 嵌入 |
| `tokenless-stats` | `tokenless-stats` | SQLite 指标追踪 |
| `core` | `tokenless-core` | 共享核心工具 |
| `tokenless-tui` | `tokenless-tui` | TUI 组件库 |
| `tokenless-cli` | `tokenless` | CLI 二进制 |
| `apps/tui` | `tokenless-tui-app` | TUI 应用二进制 |

### 发布前准备

每个 crate 需要：

**清理 Cargo.toml：**
- 补全 `description`、`keywords`、`categories`、`repository`、`homepage`
- `license = "Apache-2.0"` ✅ 已有
- 去掉 workspace 继承的 `documentation`、`readme` 等

**tokenless-cli 特殊处理：**
- 需要移除 `rtk-registry` 的 path 依赖（外部路径）
- 改为 crates.io 版本或 feature-gate
- 或先发布 rtk-registry 到 crates.io

**crates.io 依赖链：**

```
core ────────────────────┐
tokenless-schema ────────┤
tokenless-semantic ──────┤
                         ├──→ tokenless-cli (tokenless)
tokenless-stats ─────────┤
tokenless-tui ───────────┤
                         ├──→ apps/tui (tokenless-tui-app)
```

### 发布步骤

```bash
# 1. 确保已登录
cargo login

# 2. 升级版本号（workspace Cargo.toml）
#    version = "0.X.0"  （所有 crate 用 version.workspace = true）

# 3. 更新跨 crate 依赖版本
#    crates/tokenless-cli/Cargo.toml:
#      tokenless-schema = { path = "../tokenless-schema", version = "0.X.0" }
#      tokenless-stats   = { path = "../tokenless-stats",   version = "0.X.0" }

# 4. 提交 + 打 tag（tag push 触发 GitHub Release CI）
git add Cargo.toml Cargo.lock crates/tokenless-cli/Cargo.toml
git commit -m "chore: bump to 0.X.0"
git tag v0.X.0
git push origin master v0.X.0

# 5. 按依赖顺序发布 crates.io（本地手动）
cargo publish -p tokenless-schema
cargo publish -p tokenless-stats
cargo publish -p tokenless

# 6. 验证
cargo install tokenless
tokenless --version
```

> **注意**：crates.io 发布改为本地手动，GitHub Actions 不再自动发布。
> tag push 只会触发 build + GitHub Release（各平台二进制）。
> 如需通过 CI 发布 crates.io，在 Actions 中手动触发 Release workflow 并勾选 `publish-crates-io`。

### rtk-registry 依赖 ✅ 已解决

`rtk-registry v0.1.0` 已发布到 crates.io（2026-05）。
workspace 依赖已从 `path = "../rtk/crates/rtk-registry"` 改为 `"0.1.0"`。
`cargo install tokenless` 可直接安装，无需本地 RTK 仓库。

### 版本号策略

```
v0.1.0 → v0.4.0 → v1.0.0
  dev      功能完善   正式版
```

当前 `version = "0.4.0"`。

---

## 三、npm 发布（OpenClaw 插件）

### 准备

`adapters/tokenless/openclaw/package.json` 已基本 OK，需要补全：

```json
{
  "name": "@tokenless/openclaw-plugin",
  "version": "1.0.0",
  "publishConfig": {
    "access": "public"
  },
  "files": ["index.js", "openclaw.plugin.json"]
}
```

### 编译

OpenClaw 插件是 TypeScript，发布前需要编译为 JS：

```bash
cd adapters/tokenless/openclaw

# 编译 TS → JS
npx esbuild index.ts --bundle --platform=node --format=esm --outfile=index.js

# 验证
node -e "import('./index.js').then(m => console.log(m.default.id))"
# → tokenless-openclaw
```

### 发布

```bash
npm login
npm publish
```

### 用户安装方式

```bash
# 发布后
openclaw plugins install @tokenless/openclaw-plugin

# 或指定版本
openclaw plugins install @tokenless/openclaw-plugin@1.0.0
```

---

## 四、GitHub Release（二进制分发）

### CI 自动构建

创建 GitHub Actions，在 tag 时自动构建各平台二进制：

```yaml
# .github/workflows/release.yml
name: Release
on:
  push:
    tags: ["v*"]
jobs:
  build:
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo build --release
      - uses: softprops/action-gh-release@v2
        with:
          files: target/release/tokenless*
```

### Release 产物

| 平台 | 文件 |
|------|------|
| Linux x86_64 | `tokenless-x86_64-unknown-linux-gnu.tar.gz` |
| macOS arm64 | `tokenless-aarch64-apple-darwin.tar.gz` |
| macOS x86_64 | `tokenless-x86_64-apple-darwin.tar.gz` |
| Windows x86_64 | `tokenless-x86_64-pc-windows-msvc.zip` |

### Release 内容

每个 release 附带：
- 二进制文件（各平台）
- Adapter 文件（openclaw + hermes）
- tool-ready-spec.json
- tokenless-env-fix.sh
- CHANGELOG

---

## 五、Homebrew（macOS）

### 创建 tap

```bash
# 新建 tap 仓库
gh repo create TokenFleet-AI/homebrew-tap --public

# 创建 formula
# Formula/tokenless.rb
class Tokenless < Formula
  desc "LLM token optimization toolkit"
  homepage "https://github.com/TokenFleet-AI/tokenless"
  url "https://github.com/TokenFleet-AI/tokenless/releases/download/v0.4.0/tokenless-aarch64-apple-darwin.tar.gz"
  sha256 "..."
  license "Apache-2.0"

  def install
    bin.install "tokenless"
  end
end
```

### 用户安装

```bash
brew install TokenFleet-AI/tap/tokenless
```

---

## 六、发布顺序建议

### Phase 1 — MVP（1 天）

```
1. ✅ rtk-registry v0.1.0 已发布
2. tokenless-schema 发 crates.io
3. tokenless-stats 发 crates.io
4. tokenless-cli 发 crates.io
5. cargo install tokenless 验证
```

### Phase 2 — 插件（2-3 天）

```
1. 编译 index.ts → index.js
2. npm publish @tokenless/openclaw-plugin
3. 创建 GitHub Release CI
4. 创建 Homebrew tap
```

### Phase 3 — 生态（远期）

```
1. 提交 Hermes 官方插件列表
2. 创建项目网站 / 文档站
3. 写集成教程（视频 / 博客）
```

---

## 七、维护清单

### 每次发布前

- [ ] `cargo test --workspace` 全部通过
- [ ] `make lint` 全部通过
- [ ] README 更新（如果功能有变化）
- [ ] CHANGELOG 更新
- [ ] 版本号升级（`cargo release`）

### 版本号约定

| 变化 | 版本号 | 示例 |
|------|--------|------|
| 初始开发 | 0.1.x | v0.1.0 |
| 新增功能（非破坏性） | 0.2.x | v0.4.0 |
| API 重大变更 | 0.3.x / 1.0 | v0.3.0 |
| 正式版 | 1.x.x | v1.0.0 |
