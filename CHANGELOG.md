# Changelog

All notable changes to this project will be documented in this file. See [conventional commits](https://www.conventionalcommits.org/) for commit guidelines.

---
## [unreleased]

### Bug Fixes

- **(cli)** replace expect() with explicit error handling in main - ([fde2862](https://github.com/TokenFleet-AI/tokenless/commit/fde2862565124d02c3ce8d8ef15728f56c66732a)) - baoyx
- reduce tokenless-tui keywords to 5 for crates.io - ([8e34715](https://github.com/TokenFleet-AI/tokenless/commit/8e347157490370beae5054c305a6caff8d03bb29)) - baoyx
- remove --workspace from cargo release commit - ([2213cf0](https://github.com/TokenFleet-AI/tokenless/commit/2213cf069248d774458d30e4f20b12296ca04f45)) - baoyx

### Documentation

- add release guide with two-step workflow - ([95584e5](https://github.com/TokenFleet-AI/tokenless/commit/95584e50ccb08e03a8cd51704bf08ec58b6dc24b)) - baoyx

### Features

- split release into push and publish steps for CI safety - ([e803d7b](https://github.com/TokenFleet-AI/tokenless/commit/e803d7b0acbb46b6887b5b42b890859b71be0a02)) - baoyx
- integrate exception-collector for CLI error capture - ([97d849b](https://github.com/TokenFleet-AI/tokenless/commit/97d849b25f6dce0a7b19764908a4d28c1943a517)) - baoyx

### Miscellaneous Chores

- remove tless-tui standalone binary and cleanup - ([9482338](https://github.com/TokenFleet-AI/tokenless/commit/94823381bdc38d61d5a0d76d00c2d847e6f9860b)) - baoyx
- use exception-collector v0.1 from crates.io - ([d48c1ca](https://github.com/TokenFleet-AI/tokenless/commit/d48c1ca54663354a23cbc1bc75caf5efdcf8ab3e)) - baoyx
- bump MSRV to 1.96.0 - ([f7f9263](https://github.com/TokenFleet-AI/tokenless/commit/f7f9263fc4f6f0bb32e7da7550e78d8d3cc045a1)) - baoyx
- release 1.3.0 - ([4225076](https://github.com/TokenFleet-AI/tokenless/commit/422507614c4a3b0e51c252c3fecd0863d1dca984)) - baoyx

### Refactoring

- use git dependency for exception-collector - ([8d58e6b](https://github.com/TokenFleet-AI/tokenless/commit/8d58e6b25ef47cfbf502dcd9f9e983da25db757d)) - baoyx

---
## [1.2.0](https://github.com/TokenFleet-AI/tokenless/compare/v1.0.0..v1.2.0) - 2026-06-24

### Bug Fixes

- **(ci)** conditionally enable ONNX feature per target - ([2f195da](https://github.com/TokenFleet-AI/tokenless/commit/2f195dabe6ae30492f0b4f5cf2ada600943e5ddd)) - baoyx
- **(ci)** pin Xcode 16.3 on macOS to avoid broken clang_rt.osx - ([0f65c3c](https://github.com/TokenFleet-AI/tokenless/commit/0f65c3c6181980618138ebed78dd3e1f2e2d4f62)) - baoyx
- **(ci)** use macos-14 for macOS check, avoiding Xcode 16.4 clang_rt.osx breakage - ([23033b4](https://github.com/TokenFleet-AI/tokenless/commit/23033b4915a819b69a6a4439d3c68e3f1379e84d)) - baoyx
- **(ci)** auto-detect working Xcode on macOS runner - ([93a0d09](https://github.com/TokenFleet-AI/tokenless/commit/93a0d0993d8c959036b764975ffd824bb31926ec)) - baoyx
- **(ci)** drop --features onnx from cross-platform Check jobs - ([01c26d8](https://github.com/TokenFleet-AI/tokenless/commit/01c26d8d88860db73036e11c6ea221894e243eff)) - baoyx
- **(ci)** serialize tests to avoid SQLite database lock - ([5157a68](https://github.com/TokenFleet-AI/tokenless/commit/5157a68e547801b73a567806097ce6a55236ff26)) - baoyx
- **(ci)** disable ONNX for aarch64-apple-darwin in release - ([6640e7b](https://github.com/TokenFleet-AI/tokenless/commit/6640e7b021ee4bb104d43a8e686ec37ab0a96b82)) - baoyx
- **(ci)** add x86_64-unknown-linux-gnu with ONNX, refactor features matrix - ([baa17d6](https://github.com/TokenFleet-AI/tokenless/commit/baa17d6f1136f96415983302e5d45cb4a36b5e07)) - baoyx
- **(tests)** update insta snapshots after version bump - ([ea69caa](https://github.com/TokenFleet-AI/tokenless/commit/ea69caab884141a90d474b82b4e8d7611e1c7103)) - baoyx
- **(tests)** normalize environment-specific output in e2e snapshots - ([2d6983d](https://github.com/TokenFleet-AI/tokenless/commit/2d6983da95755ff63a627ee4f5290bb1afd01f2b)) - baoyx
- **(tests)** add UTF-8 boundary truncation test from PR #2 - ([129af02](https://github.com/TokenFleet-AI/tokenless/commit/129af0265a7a736790004c480a14bd7eb68af6ca)) - baoyx
- **(tests)** use workspace root instead of crate dir for CWD replacement - ([0f2a507](https://github.com/TokenFleet-AI/tokenless/commit/0f2a5075746dce3cc7c8fadc3a8aad854762c19a)) - baoyx
- **(tests)** strip RTK tip instead of replacing with placeholder - ([b4ff5b6](https://github.com/TokenFleet-AI/tokenless/commit/b4ff5b6be7396711d8708250d881bab4807eb259)) - baoyx
- clippy pedantic 合规 — 借用替代移动、移除死代码、writeln! 替代 push_str - ([08f17e5](https://github.com/TokenFleet-AI/tokenless/commit/08f17e5619bd29318d0a819a347a6e99191ef146)) - baoyx
- CI 失败修复 — coverage snapshot 路径规范化 + Windows 编译隔离 - ([0abe018](https://github.com/TokenFleet-AI/tokenless/commit/0abe018d8971289d2801a1a273ffa11b080dcad9)) - baoyx

### Features

- 多角色并行分析 + 30项功能规划全面落地 - ([8c3ffe8](https://github.com/TokenFleet-AI/tokenless/commit/8c3ffe87d6f2cec5e53bdb0e7ab608f01b38bac3)) - baoyx

### Miscellaneous Chores

- **(deps)** bump rtk-registry to 1.0.0 and tokenizers to 0.23 - ([2d49125](https://github.com/TokenFleet-AI/tokenless/commit/2d491251f93eba6931df7443821773fdb60677ba)) - baoyx
- **(release)** bump workspace to 1.2.0 - ([04d82ab](https://github.com/TokenFleet-AI/tokenless/commit/04d82ab3e1a9fde50801f78091e3d09bb6b2f5b1)) - baoyx
- bump version to 1.1.0 - ([95dbf17](https://github.com/TokenFleet-AI/tokenless/commit/95dbf17fa280a70b20413ad696474effa02847ff)) - baoyx
- gitignore 添加 lcov.info (coverage 产物) - ([f71cfe7](https://github.com/TokenFleet-AI/tokenless/commit/f71cfe749aefa431b45071ab11945ea940b90899)) - baoyx
- remove stale test_allow Mach-O binaries, add to .gitignore - ([0592ff7](https://github.com/TokenFleet-AI/tokenless/commit/0592ff76d601575a0dfe7de2638de50da572dff6)) - baoyx

### Other

- Merge branch 'master' of github.com:TokenFleet-AI/tokenless

# Conflicts:
#	crates/tokenless-cli/src/commands/hook.rs - ([1be5767](https://github.com/TokenFleet-AI/tokenless/commit/1be5767569327757b7238274c1fa143ba731e7f0)) - baoyx

---
## [1.0.0](https://github.com/TokenFleet-AI/tokenless/compare/v0.4.0..v1.0.0) - 2026-06-12

### Bug Fixes

- CI failures — test ordering, cargo-audit, release permissions - ([f939533](https://github.com/TokenFleet-AI/tokenless/commit/f9395336055f32595ba8ecdf9cf026511462415b)) - baoyx
- 修复跨平台 CI 测试失败 - ([a348891](https://github.com/TokenFleet-AI/tokenless/commit/a34889194e4484bfbba9651cab26ba96b2ea8e12)) - baoyx
- 删除 query.rs 多余尾随逗号 - ([b675053](https://github.com/TokenFleet-AI/tokenless/commit/b675053ee8fed9755a0bc772b03a66aa41485c19)) - baoyx
- sort_by → sort_by_key (clippy unnecessary_sort_by, Rust 1.96) - ([3637558](https://github.com/TokenFleet-AI/tokenless/commit/3637558c7c323729dbdd899d4a00806fa35ff370)) - baoyx
- unwrap → expect in env_check/mod.rs (clippy unwrap_in_result, Rust 1.96) - ([4ebf197](https://github.com/TokenFleet-AI/tokenless/commit/4ebf197ddf8842c3a8fa85daf21a6cca85d9d02f)) - baoyx
- expect → map_err+? (clippy unwrap_in_result 也禁止 expect, Rust 1.96) - ([d0b1b2f](https://github.com/TokenFleet-AI/tokenless/commit/d0b1b2f93550992e9f96dc6ed7e5019da8f01889)) - baoyx
- IdentitySource 改用 #[derive(Default)] 替代手动 impl - ([d35f147](https://github.com/TokenFleet-AI/tokenless/commit/d35f147bba20d4d659e0849a926c94a1bd869c6a)) - baoyx
- typos 和 check-agent-sync 仅在 Linux runner 执行 - ([349cb18](https://github.com/TokenFleet-AI/tokenless/commit/349cb180b825b1492d2bbff8c25a2e07b56fb38d)) - baoyx
- README 开源协议链接指向 LICENSE.md - ([861786e](https://github.com/TokenFleet-AI/tokenless/commit/861786e5eea419730abff3c9ff7eb007ab5329c0)) - baoyx
- README.zh 开源协议链接也指向 LICENSE.md - ([f4568d0](https://github.com/TokenFleet-AI/tokenless/commit/f4568d0b17c0199bb03eec50e3c970fc40ad17e3)) - baoyx
- License badge 改用静态 URL 替代 GitHub API 动态检测 - ([6ba606c](https://github.com/TokenFleet-AI/tokenless/commit/6ba606c800b3083781fbcc34460c5891de0a16d9)) - baoyx
- reduce keywords to 5 for crates.io publish - ([4b2a199](https://github.com/TokenFleet-AI/tokenless/commit/4b2a1999bd294d7d79394d1e49f5c1a515c532a1)) - baoyx
- add attestations write permission for release workflow - ([fb10d97](https://github.com/TokenFleet-AI/tokenless/commit/fb10d97c28ca15cd028e6b1d266b9ecad55691e0)) - baoyx
- remove broken attestation step from release workflow - ([4a66960](https://github.com/TokenFleet-AI/tokenless/commit/4a66960faf5fe9f4a4afa50629fa9a4d1628afd6)) - baoyx
- compress_plain_text panics on UTF-8 character boundary (#1) - ([6575315](https://github.com/TokenFleet-AI/tokenless/commit/65753156ce9088dff1f943201347aa1346932f54)) - baoyx

### Documentation

- 修复 6 个 crate README 的代码示例和 API 覆盖 - ([c87c23c](https://github.com/TokenFleet-AI/tokenless/commit/c87c23c61e16d3c588cb7bb6e54b6f8d10c12254)) - baoyx
- P2 polish — badges, why sections, keywords/categories for all crates - ([868de02](https://github.com/TokenFleet-AI/tokenless/commit/868de022ed217d53c9ba4f69a771d79b5e481700)) - baoyx
- 用户指南新增多项目支持和实验功能专题章节 - ([e03391b](https://github.com/TokenFleet-AI/tokenless/commit/e03391b4db6e93e049ed4f76715a1d1eb06bbe22)) - baoyx
- 添加微信开发者群二维码（README + 用户指南） - ([4e3dd31](https://github.com/TokenFleet-AI/tokenless/commit/4e3dd31e59a5a3495e4f803d122809719fabaa77)) - baoyx
- README.zh.md 补充开发者社区章节 - ([6674930](https://github.com/TokenFleet-AI/tokenless/commit/6674930f2957747e37e071957032040b4c73c439)) - baoyx
- README 头部对齐 llm-bridge-rust 风格 — badges + logo + 标题 + tagline - ([97ab2e1](https://github.com/TokenFleet-AI/tokenless/commit/97ab2e1e25b1e2dfc9436181c8305a0a3f8abc34)) - baoyx
- 修复 README 审核发现的 15+ 问题 - ([c821d95](https://github.com/TokenFleet-AI/tokenless/commit/c821d952d1e4defb0a2886abc6bcefe64898a604)) - baoyx
- README init 章节补充项目级 vs 全局级说明 - ([72317d6](https://github.com/TokenFleet-AI/tokenless/commit/72317d6a524b76dd47dbfd8c1da6301bbe997345)) - baoyx

### Features

- 添加品牌图片 assets/tokenless.jpg 并已在 README 中引用 - ([60e053e](https://github.com/TokenFleet-AI/tokenless/commit/60e053ee2b7ec14796349f0ed7ac7202f7f7db12)) - baoyx
- README 和 README.zh.md 添加品牌图片引用 - ([c8f3af9](https://github.com/TokenFleet-AI/tokenless/commit/c8f3af973ad4737971d5da02f0f4e29b1aaa57d7)) - baoyx
- 添加 compression stats reporting spec (0018)，替换微信开发群图片 - ([ef9bf98](https://github.com/TokenFleet-AI/tokenless/commit/ef9bf9897477eca685e55bb2ffd83a6ba21c982e)) - baoyx
- tokenless 品牌设计最终版 — CSS 动画 logo + 暗色变体 + 全 crate 统一 - ([4f51659](https://github.com/TokenFleet-AI/tokenless/commit/4f516597a5bb0fdc97dc5c37a46a1a1d8330e4eb)) - baoyx
- agent-proxy 上报集成 + 目录迁移至 .tokenfleet-ai/tokenless - ([8ebfed6](https://github.com/TokenFleet-AI/tokenless/commit/8ebfed6179ac12819b29e9c9a58962ff01056bb3)) - baoyx
- tokenless init 三功能 — 压缩控制 + 用户检测 + passthrough 观察模式 - ([e252f39](https://github.com/TokenFleet-AI/tokenless/commit/e252f39577c2af8e992df1f2184768fb240dab8d)) - baoyx
- record_compression_stats 添加 user_name 参数，纳入 ProxyReport - ([928fc0a](https://github.com/TokenFleet-AI/tokenless/commit/928fc0ae24279f029cee63be598cbecdd272e73e)) - baoyx
- init 支持 --user-name，全局安装不再写死 --project - ([77c9d45](https://github.com/TokenFleet-AI/tokenless/commit/77c9d45c7b1cc2b28b744ca7037aef59a832b27d)) - baoyx
- init 支持 Codex CLI — AGENTS.md + RTK.md 规则文件 - ([a0eac01](https://github.com/TokenFleet-AI/tokenless/commit/a0eac0183d87347642804080c4f4bf488f0b60be)) - baoyx

### Miscellaneous Chores

- exclude tless-tui binary from crates.io publish - ([7a735c7](https://github.com/TokenFleet-AI/tokenless/commit/7a735c7c73da8c8c739a5ef77319c21437ca0cad)) - baoyx
- 添加 .DS_Store 到 .gitignore - ([6d5bf63](https://github.com/TokenFleet-AI/tokenless/commit/6d5bf6363f7e187dbf5c74e2f6f097627f9564d4)) - baoyx
- rust-toolchain 升级到 stable，与 CI 对齐 - ([e41957b](https://github.com/TokenFleet-AI/tokenless/commit/e41957b724361644eaa2f71242e67fdcb1d2c06d)) - baoyx
- pre-commit 和 CI 对齐 — 互补遗漏检查项 - ([589e973](https://github.com/TokenFleet-AI/tokenless/commit/589e97304e233d29bb4dec08ecd97238920ce742)) - baoyx
- bump version to 1.0.0 - ([9343811](https://github.com/TokenFleet-AI/tokenless/commit/934381137add5f72e0884744973a17f287b39c0f)) - baoyx

### Tests

- 更新 golden snapshots — 适配 SchemaCompressor compress_all 变更 - ([97e9f20](https://github.com/TokenFleet-AI/tokenless/commit/97e9f207697b5b7c7a8e6dfde3802539db71fa8d)) - baoyx
- 更新 golden snapshots — 适配 JSON key 排序变化 - ([9e9e824](https://github.com/TokenFleet-AI/tokenless/commit/9e9e824434dc3b8572bc86b360b7d57dedbf9bf9)) - baoyx

---
## [0.4.0](https://github.com/TokenFleet-AI/tokenless/compare/models-v1..v0.4.0) - 2026-06-02

### Bug Fixes

- pedantic per-crate strategy - ([e13005e](https://github.com/TokenFleet-AI/tokenless/commit/e13005e98938efec96250a0e0d7472c7e62df447)) - baoyx
- restore pedantic to pre-commit - ([882a607](https://github.com/TokenFleet-AI/tokenless/commit/882a607b0d2228827ad372a0b9f83d04eb3f4d00)) - baoyx
- gitignore ONNX model files (use GitHub Release models-v1) - ([34c90b0](https://github.com/TokenFleet-AI/tokenless/commit/34c90b003f315ab550d12dd91f38eef0553bdd83)) - baoyx
- deny.toml skip/skip-tree moved out of [[bans.features]] - ([bc98c15](https://github.com/TokenFleet-AI/tokenless/commit/bc98c1555b3add566fe3e329645426bdfdc70260)) - baoyx
- deny.toml allow workspace crates for wildcard ban - ([fe42f38](https://github.com/TokenFleet-AI/tokenless/commit/fe42f382195b0ea3bdc6eb5535704e66557d5289)) - baoyx
- deny.toml structure + wildcard deps add version - ([f2db01d](https://github.com/TokenFleet-AI/tokenless/commit/f2db01d22b4571ade0e8511867bb46bc90437e03)) - baoyx

### Documentation

- 补齐 tokenless-core/semantic/tui 的 README.md - ([69b9d96](https://github.com/TokenFleet-AI/tokenless/commit/69b9d9693e37af8c0c6eefed63a40515b690d65b)) - baoyx

### Features

- v0.4.0 全量改造 - ([6623ca5](https://github.com/TokenFleet-AI/tokenless/commit/6623ca59167ed7f09ee30cf6af7c4e4d8ced1afe)) - baoyx
- v0.4.0 全量改造 — 多项目支持 + 实验模式 + 文档对齐 - ([8b6a576](https://github.com/TokenFleet-AI/tokenless/commit/8b6a576378652de4a961155699f9fd4626f3f082)) - baoyx

### Miscellaneous Chores

- add model update script - ([a1b2b41](https://github.com/TokenFleet-AI/tokenless/commit/a1b2b41d9b220a199f3625cdf5dea8532c8bb1f1)) - baoyx
- 移除 rustfmt.toml nightly-only 选项，pre-commit 改用 stable fmt - ([dea8759](https://github.com/TokenFleet-AI/tokenless/commit/dea87595c412bcf94ed77a3f71ccb9f41f1a6be6)) - baoyx
- add cargo-release config with publish order - ([95784da](https://github.com/TokenFleet-AI/tokenless/commit/95784da259ff0575aa3dab7c23e5fc3ab2f6fc91)) - baoyx
- fix cargo-release 0.25.20 config, Makefile 支持 VERSION 参数 - ([031c53f](https://github.com/TokenFleet-AI/tokenless/commit/031c53f77967f7febc9a64973353a4b56e69439f)) - baoyx
- 移除 CI 中 crates.io 自动发布，改由本地 make release 负责 - ([407a31a](https://github.com/TokenFleet-AI/tokenless/commit/407a31a3eb5ccdc9d538b8b331c25450746e7158)) - baoyx
- update CHANGELOG for v0.4.0, fix cliff repo URL - ([169e9d0](https://github.com/TokenFleet-AI/tokenless/commit/169e9d04fa87bab32680bc8d8f1a54474dba723c)) - baoyx

---
## [models-v1](https://github.com/TokenFleet-AI/tokenless/compare/v0.3.1..models-v1) - 2026-06-01

### Bug Fixes

- move RTK_SKIP_HOOK_CHECK from command string to settings.json env block - ([89ffe5d](https://github.com/TokenFleet-AI/tokenless/commit/89ffe5d316b86d8e9cec5258eb5051624f55d8be)) - baoyx
- Schema security hardening + Response compression profiles - ([76dc6a6](https://github.com/TokenFleet-AI/tokenless/commit/76dc6a6a6610aa9a78b3ca914f60feff3574d69a)) - baoyx

### Documentation

- add dev-install.sh script and document it - ([8d87e84](https://github.com/TokenFleet-AI/tokenless/commit/8d87e846e662cfed54bcf83c7508eb0ca2b9f939)) - baoyx
- add dev install path guidance to agent guide - ([84e810e](https://github.com/TokenFleet-AI/tokenless/commit/84e810ed79c004fa214796bdd8e58a5b47d442f4)) - baoyx

### Features

- add compression correctness verification mechanisms (P0) - ([e41ed83](https://github.com/TokenFleet-AI/tokenless/commit/e41ed8383410df348d3d99cd3dd60bffeac67ff8)) - baoyx

### Miscellaneous Chores

- update 22 third-party dependencies to latest versions - ([b50d6a3](https://github.com/TokenFleet-AI/tokenless/commit/b50d6a3315f3878c2a87c70f88a701ff4c8823aa)) - baoyx
- update main.rs, stats lib and query modules - ([b4f8db8](https://github.com/TokenFleet-AI/tokenless/commit/b4f8db86921df95762a25f3939a4ea0b2f7a423f)) - baoyx

---
## [0.3.1](https://github.com/TokenFleet-AI/tokenless/compare/v0.3.0..v0.3.1) - 2026-05-30

### Documentation

- update specs to reflect v0.3.0 implementation status - ([084369b](https://github.com/TokenFleet-AI/tokenless/commit/084369bdab58373fa43db8a0032dbd8a9256f5c2)) - baoyx

### Features

- CLI UX enhancements v0.3.1 - ([e0b0b16](https://github.com/TokenFleet-AI/tokenless/commit/e0b0b16a92d55fc62d36ee13d8520c76edd2bba0)) - baoyx

### Miscellaneous Chores

- v0.3.0 release - ([75c73c2](https://github.com/TokenFleet-AI/tokenless/commit/75c73c2d865aefb27be78e76452dddb6edea8861)) - baoyx
- bump workspace version to 0.3.1 - ([15a9ecf](https://github.com/TokenFleet-AI/tokenless/commit/15a9ecf2b08b00910cce0eda7073df636b12630d)) - baoyx

### Other

- Prepend RTK_SKIP_HOOK_CHECK=1 to rewritten commands

Suppress RTK's "No hook installed" warning in tokenless hook output
by setting the env var on all rtk commands before execution.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com> - ([3c58a4c](https://github.com/TokenFleet-AI/tokenless/commit/3c58a4c9e29683122a9596d91efbdc6dffb1b880)) - baoyx
- Add wechat Toutiao marketing copy and update docs index

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com> - ([183d1f3](https://github.com/TokenFleet-AI/tokenless/commit/183d1f39aaadac3f211b99a235428c9235a1e394)) - baoyx

---
## [0.3.0](https://github.com/TokenFleet-AI/tokenless/compare/v0.2.0..v0.3.0) - 2026-05-27

### Documentation

- update README and indexes for new features - ([2b239c2](https://github.com/TokenFleet-AI/tokenless/commit/2b239c2a91f4b0030829c2cb5826a2ba060fb0b3)) - baoyx

### Features

- comprehensive optimization, TDD bugfixes, design docs, and predictive cache - ([c3f10eb](https://github.com/TokenFleet-AI/tokenless/commit/c3f10eb445a2d178f3958e3d2887cbfdde33ffb8)) - baoyx
- MCP server + intelligent format router - ([372676f](https://github.com/TokenFleet-AI/tokenless/commit/372676fd001b5996bcfaa8662d7f5814bd9094ab)) - baoyx
- differential response compression for polling-style tool calls - ([16c3d35](https://github.com/TokenFleet-AI/tokenless/commit/16c3d35912ff44d6cf45cdbadfa6150281e82d93)) - baoyx

### Other

- Remove crates.io publishing from GitHub Actions release workflow

Crates.io publishing is now done manually, not automatically on tag push. - ([853257a](https://github.com/TokenFleet-AI/tokenless/commit/853257a4e2519913ecffed8975037601b4069391)) - baoyx
- Gate crates.io publishing behind publish-crates-io flag

Default to false: tag push (CD) only builds + GitHub Release.
Manual workflow_dispatch with --publish-crates-io to publish to crates.io. - ([9558bf7](https://github.com/TokenFleet-AI/tokenless/commit/9558bf7bce33625d48116871c8e8bcac7cfef8e7)) - baoyx
- Update release-plan.md with current version and manual publish steps - ([8d658f3](https://github.com/TokenFleet-AI/tokenless/commit/8d658f32039ee3cca63c12dace5713446b4934ca)) - baoyx
- Fix rtk-registry links to use master branch

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com> - ([214c74d](https://github.com/TokenFleet-AI/tokenless/commit/214c74d1f75c18e33e5c6cbd47a57ca9cd2ed745)) - baoyx

---
## [0.2.0](https://github.com/TokenFleet-AI/tokenless/compare/v0.1.1..v0.2.0) - 2026-05-26

### Miscellaneous Chores

- v0.2.0 - ([006ca3f](https://github.com/TokenFleet-AI/tokenless/commit/006ca3fa29498ec76aa7f71dc817fb061c2d7c9d)) - baoyx

### Other

- Add rewrite-command stats recording and breakdown

- Wire record_compression_stats into the Rewrite command handler
- Skip meaningless before/after token comparison for RewriteCommand
- Add `tokenless stats rewrites` subcommand grouped by original command - ([34469a1](https://github.com/TokenFleet-AI/tokenless/commit/34469a158c131405ba68c043336c05381d58c8a6)) - baoyx
- Add savings percentage and pagination to stats rewrites

- Show estimated savings % for each rewritten command via classify_command
- Add --offset flag for pagination with page indicator
- Default limit changed from 50 to 20
- Add .codegraph/ to .gitignore - ([1ffbdf2](https://github.com/TokenFleet-AI/tokenless/commit/1ffbdf2496fe1431d2370110c8368f5d38b35f68)) - baoyx
- Always show page indicator in stats rewrites output - ([10f88f7](https://github.com/TokenFleet-AI/tokenless/commit/10f88f7ea0b18e9a19c38c55e0e03145b4c8e901)) - baoyx
- Add Claude Code PreToolUse hook with JSON stdin/stdout protocol

- Add rewrite-hook subcommand for JSON-based Claude Code hook protocol
- Install shell script wrapper for jq-based JSON parsing in hook context
- Fix init to write hook script alongside settings.json for Claude Code
- Preserve existing shell-script hooks for Cursor, Windsurf, and other agents - ([ce14e01](https://github.com/TokenFleet-AI/tokenless/commit/ce14e01eecb939fe4732bcde4e89085459b6b91d)) - baoyx
- Fix rewrite-hook updatedInput format to match Claude Code protocol

The updatedInput must contain the tool_input fields directly (e.g. {"command":
"rtk git status"}), not nested inside another "tool_input" key. This matches
rtk's output format. Also simplify init to use inline command directly. - ([ae402c8](https://github.com/TokenFleet-AI/tokenless/commit/ae402c8927181febace143a942e4b4a351250e0d)) - baoyx

---
## [0.1.1](https://github.com/TokenFleet-AI/tokenless/compare/v0.1.0..v0.1.1) - 2026-05-25

### Bug Fixes

- add version spec to path deps for crates.io publish - ([ad18e3d](https://github.com/TokenFleet-AI/tokenless/commit/ad18e3dc4ba6a52e1ce0c07a8711ae2b09b3b1c3)) - baoyx
- use rustup target add in release workflow, fix CI fmt - ([1df6cc6](https://github.com/TokenFleet-AI/tokenless/commit/1df6cc61c82a03b697002543386a2b0074d101f1)) - baoyx
- release workflow - don't block GitHub Release on crates-io - ([8b0276d](https://github.com/TokenFleet-AI/tokenless/commit/8b0276ddf7d68cd3a50f98a6f79a814073291f44)) - baoyx

### Documentation

- add README.md for each crate - ([59fec5f](https://github.com/TokenFleet-AI/tokenless/commit/59fec5fc0e55d181a5f8edffe93f095f71d0d8b9)) - baoyx

### Miscellaneous Chores

- bump to v0.1.1 - ([f711d54](https://github.com/TokenFleet-AI/tokenless/commit/f711d54a92de703261511d3b6312cf1e57c5e135)) - baoyx

---
## [0.1.0] - 2026-05-25

### Bug Fixes

- use CARGO_REGISTRY_TOKEN env instead of cargo login - ([0e3ebfc](https://github.com/TokenFleet-AI/tokenless/commit/0e3ebfc25646887d564b7d430df4a74e2d1676ba)) - baoyx

### Features

- add release CI, OpenClaw/Hermes plugins, init command, user guide - ([24547c2](https://github.com/TokenFleet-AI/tokenless/commit/24547c25da98438b6b6f74f5aa4c3120f42155af)) - baoyx

### Other

- init prj - ([d77a4af](https://github.com/TokenFleet-AI/tokenless/commit/d77a4af7f3fa5fa90049a9038073ae308f1c2a0d)) - baoyx
- Implement tokenless core crates: schema, stats, cli

- tokenless-schema: SchemaCompressor + ResponseCompressor for LLM token optimization
- tokenless-stats: SQLite-based compression metrics tracking
- tokenless-cli: CLI binary with compress-schema, compress-response, compress-toon,
  decompress-toon, env-check, stats, and rewrite subcommands
- rtk-registry integration for command rewriting
- tool-ready-spec.json and tokenless-env-fix.sh for environment checks
- Update license to Apache 2.0, restructure workspace deps
- Architecture design docs (EN + CN) - ([7020987](https://github.com/TokenFleet-AI/tokenless/commit/7020987a8faf3e2afb3ab754bf3a0edd9b622669)) - baoyx

<!-- generated by git-cliff -->
