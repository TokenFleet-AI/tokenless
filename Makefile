BIN_DIR ?= $(HOME)/.local/bin
ADAPTER_DIR ?= $(HOME)/.local/share/anolisa/adapters/tokenless

build:
	@cargo build --release

test:
	@cargo test

fmt:
	@cargo fmt 2>&1

clippy:
	@cargo clippy --all-targets --all-features -- -D warnings

lint: fmt clippy

install: build
	@mkdir -p $(BIN_DIR)
	@cp target/release/tokenless $(BIN_DIR)/tokenless
	@echo "Installed tokenless to $(BIN_DIR)/tokenless"

adapter-install:
	@mkdir -p $(ADAPTER_DIR)/common
	@cp -r adapters/tokenless/common/* $(ADAPTER_DIR)/common/
	@echo "Installed adapters to $(ADAPTER_DIR)"

adapter-uninstall:
	@rm -rf $(ADAPTER_DIR)
	@echo "Removed $(ADAPTER_DIR)"

setup: install adapter-install
	@echo "tokenless setup complete"

clean:
	@cargo clean

check-agent-sync:
	@test -f CLAUDE.md || { \
		echo "CLAUDE.md is required for project-level agent instructions."; \
		exit 1; \
	}

release:
	@cargo release tag --execute
	@git cliff -o CHANGELOG.md
	@git commit -a -n -m "Update CHANGELOG.md" || true
	@git push origin master
	@cargo release push --execute

update-submodule:
	@git submodule update --init --recursive --remote

.PHONY: build test fmt clippy lint install adapter-install adapter-uninstall setup clean check-agent-sync release update-submodule
