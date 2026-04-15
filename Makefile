PREFIX ?= /usr/local

.PHONY: install install-tui uninstall build build-tui test test-tui test-rust test-rust-integration

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
