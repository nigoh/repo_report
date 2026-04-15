PREFIX ?= /usr/local
VERSION ?= $(shell cargo metadata --no-deps --format-version 1 | python3 -c "import sys,json; print(json.load(sys.stdin)['packages'][0]['version'])")

.PHONY: install install-tui uninstall build build-tui test test-tui test-rust test-rust-integration dist

# Bash version (original)
install:
	install -m0755 bin/repo-report $(PREFIX)/bin/repo-report

uninstall:
	rm -f $(PREFIX)/bin/repo-report
	rm -f $(PREFIX)/bin/repo-report-tui

# Ratatui/Rust version
build-tui:
	cargo build --release

install-tui: build-tui
	install -m0755 target/release/repo-report-tui $(PREFIX)/bin/repo-report-tui

test:
	@bash -n bin/repo-report && echo "syntax OK"
	@bash tests/test_noninteractive.sh
	@bash tests/test_repo_detection.sh
	@bash tests/test_repo_commands.sh

test-tui:
	@echo "Running TUI key tests (requires a real TTY)..."
	@bash tests/test_tui_keys.sh

test-rust:
	cargo test
	@bash tests/test_rust_noninteractive.sh

test-rust-integration:
	@bash tests/test_rust_noninteractive.sh

# Create a distributable tar.gz for the current platform
dist: build-tui
	$(eval ARCH := $(shell uname -m))
	$(eval OS := $(shell uname -s | tr '[:upper:]' '[:lower:]'))
	$(eval STAGING := repo-report-v$(VERSION)-$(OS)-$(ARCH))
	$(eval ARCHIVE := $(STAGING).tar.gz)
	mkdir -p dist/$(STAGING)
	cp target/release/repo-report-tui dist/$(STAGING)/
	cp bin/repo-report dist/$(STAGING)/
	cp README.md README.ja.md dist/$(STAGING)/
	tar czf dist/$(ARCHIVE) -C dist $(STAGING)
	rm -rf dist/$(STAGING)
	@echo "Created dist/$(ARCHIVE)"
