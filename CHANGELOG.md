# Changelog

All notable changes to this project will be documented in this file. See [conventional commits](https://www.conventionalcommits.org/) for commit guidelines.

---
## [unreleased]

### Bug Fixes

- move RTK_SKIP_HOOK_CHECK from command string to settings.json env block - ([89ffe5d](https://github.com/TokenFleet-AI/tokenless/commit/89ffe5d316b86d8e9cec5258eb5051624f55d8be)) - baoyx
- Schema security hardening + Response compression profiles - ([76dc6a6](https://github.com/TokenFleet-AI/tokenless/commit/76dc6a6a6610aa9a78b3ca914f60feff3574d69a)) - baoyx
- pedantic per-crate strategy - ([e13005e](https://github.com/TokenFleet-AI/tokenless/commit/e13005e98938efec96250a0e0d7472c7e62df447)) - baoyx
- restore pedantic to pre-commit - ([882a607](https://github.com/TokenFleet-AI/tokenless/commit/882a607b0d2228827ad372a0b9f83d04eb3f4d00)) - baoyx
- gitignore ONNX model files (use GitHub Release models-v1) - ([34c90b0](https://github.com/TokenFleet-AI/tokenless/commit/34c90b003f315ab550d12dd91f38eef0553bdd83)) - baoyx
- deny.toml skip/skip-tree moved out of [[bans.features]] - ([bc98c15](https://github.com/TokenFleet-AI/tokenless/commit/bc98c1555b3add566fe3e329645426bdfdc70260)) - baoyx
- deny.toml allow workspace crates for wildcard ban - ([fe42f38](https://github.com/TokenFleet-AI/tokenless/commit/fe42f382195b0ea3bdc6eb5535704e66557d5289)) - baoyx
- deny.toml structure + wildcard deps add version - ([f2db01d](https://github.com/TokenFleet-AI/tokenless/commit/f2db01d22b4571ade0e8511867bb46bc90437e03)) - baoyx

### Documentation

- add dev-install.sh script and document it - ([8d87e84](https://github.com/TokenFleet-AI/tokenless/commit/8d87e846e662cfed54bcf83c7508eb0ca2b9f939)) - baoyx
- add dev install path guidance to agent guide - ([84e810e](https://github.com/TokenFleet-AI/tokenless/commit/84e810ed79c004fa214796bdd8e58a5b47d442f4)) - baoyx
- 补齐 tokenless-core/semantic/tui 的 README.md - ([69b9d96](https://github.com/TokenFleet-AI/tokenless/commit/69b9d9693e37af8c0c6eefed63a40515b690d65b)) - baoyx

### Features

- add compression correctness verification mechanisms (P0) - ([e41ed83](https://github.com/TokenFleet-AI/tokenless/commit/e41ed8383410df348d3d99cd3dd60bffeac67ff8)) - baoyx
- v0.4.0 全量改造 - ([6623ca5](https://github.com/TokenFleet-AI/tokenless/commit/6623ca59167ed7f09ee30cf6af7c4e4d8ced1afe)) - baoyx
- v0.4.0 全量改造 — 多项目支持 + 实验模式 + 文档对齐 - ([8b6a576](https://github.com/TokenFleet-AI/tokenless/commit/8b6a576378652de4a961155699f9fd4626f3f082)) - baoyx

### Miscellaneous Chores

- update 22 third-party dependencies to latest versions - ([b50d6a3](https://github.com/TokenFleet-AI/tokenless/commit/b50d6a3315f3878c2a87c70f88a701ff4c8823aa)) - baoyx
- update main.rs, stats lib and query modules - ([b4f8db8](https://github.com/TokenFleet-AI/tokenless/commit/b4f8db86921df95762a25f3939a4ea0b2f7a423f)) - baoyx
- add model update script - ([a1b2b41](https://github.com/TokenFleet-AI/tokenless/commit/a1b2b41d9b220a199f3625cdf5dea8532c8bb1f1)) - baoyx
- 移除 rustfmt.toml nightly-only 选项，pre-commit 改用 stable fmt - ([dea8759](https://github.com/TokenFleet-AI/tokenless/commit/dea87595c412bcf94ed77a3f71ccb9f41f1a6be6)) - baoyx
- add cargo-release config with publish order - ([95784da](https://github.com/TokenFleet-AI/tokenless/commit/95784da259ff0575aa3dab7c23e5fc3ab2f6fc91)) - baoyx
- fix cargo-release 0.25.20 config, Makefile 支持 VERSION 参数 - ([031c53f](https://github.com/TokenFleet-AI/tokenless/commit/031c53f77967f7febc9a64973353a4b56e69439f)) - baoyx
- 移除 CI 中 crates.io 自动发布，改由本地 make release 负责 - ([407a31a](https://github.com/TokenFleet-AI/tokenless/commit/407a31a3eb5ccdc9d538b8b331c25450746e7158)) - baoyx

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
