build:
	@cargo build

test:
	@cargo nextest run --all-features

fmt:
	@cargo +nightly fmt -- --check

clippy:
	@cargo clippy --all-targets --all-features -- -D warnings

lint: fmt clippy

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

.PHONY: build test fmt clippy lint check-agent-sync release update-submodule
