BIN_DIR ?= $(HOME)/.local/bin
ADAPTER_DIR ?= $(HOME)/.local/share/anolisa/adapters/tokenless

build:
	@cargo build --release --features tokenless-semantic/onnx

test:
	@cargo test

fmt:
	@cargo fmt 2>&1

clippy:
	@cargo clippy --all-targets --all-features -- -D warnings

audit:
	@cargo audit

lint: fmt clippy audit

install: build models-install
	@mkdir -p $(BIN_DIR)
	@cp target/release/tokenless $(BIN_DIR)/tokenless
	@echo "Installed tokenless to $(BIN_DIR)/tokenless"

models-install:
	@MODEL_DIR="$${HOME}/.tokenfleet-ai/tokenless/models"; \
	SRC="crates/tokenless-semantic/models"; \
	if [ -f "$${SRC}/all-MiniLM-L6-v2.onnx" ] && [ -f "$${SRC}/tokenizer.json" ]; then \
		mkdir -p "$${MODEL_DIR}"; \
		cp "$${SRC}/all-MiniLM-L6-v2.onnx" "$${MODEL_DIR}/"; \
		cp "$${SRC}/tokenizer.json" "$${MODEL_DIR}/"; \
		echo "Installed ONNX models to $${MODEL_DIR}"; \
	fi


adapter-install:
	@mkdir -p $(ADAPTER_DIR)/common
	@cp -r adapters/tokenless/common/* $(ADAPTER_DIR)/common/
	@cp -r adapters/tokenless/openclaw $(ADAPTER_DIR)/openclaw 2>/dev/null || true
	@cp -r adapters/tokenless/hermes $(ADAPTER_DIR)/hermes 2>/dev/null || true
	@echo "Installed adapters to $(ADAPTER_DIR)"

adapter-uninstall:
	@rm -rf $(ADAPTER_DIR)
	@echo "Removed $(ADAPTER_DIR)"

openclaw-install:
	@bash adapters/tokenless/openclaw/scripts/install.sh

openclaw-uninstall:
	@bash adapters/tokenless/openclaw/scripts/uninstall.sh

hermes-install:
	@bash adapters/tokenless/hermes/scripts/install.sh

hermes-uninstall:
	@bash adapters/tokenless/hermes/scripts/uninstall.sh

setup: install adapter-install
	@echo "tokenless setup complete"

clean:
	@cargo clean

check-agent-sync:
	@test -f CLAUDE.md || { \
		echo "CLAUDE.md is required for project-level agent instructions."; \
		exit 1; \
	}

release: release-push ## Usage: make release VERSION=patch|minor|major (step 1: push + tag)
	@echo ""
	@echo "==> Step 1 完成: 代码已推送并创建 tag"
	@echo "==> 请等待 GitHub Actions CI 通过"
	@echo "==> 查看 CI 状态: gh run list --limit 1"
	@echo "==> CI 通过后执行: make release-publish"

release-push: ## Step 1: 更新版本、提交、生成 CHANGELOG、创建 tag、推送
ifndef VERSION
	$(error Usage: make release-push VERSION=patch|minor|major)
endif
	@cargo release version $(VERSION) --execute --workspace --no-confirm
	@cargo release commit --execute --no-confirm
	@git cliff -o CHANGELOG.md
	@git commit -a -n -m "Update CHANGELOG.md" || true
	@cargo release tag --execute --workspace --no-confirm
	@git push origin master --tags

release-publish: ## Step 2: 发布到 crates.io（CI 通过后执行）
	@cargo release publish --execute --workspace --no-confirm

update-submodule:
	@git submodule update --init --recursive --remote

.PHONY: build test fmt clippy audit lint install adapter-install adapter-uninstall setup clean check-agent-sync release release-push release-publish update-submodule
