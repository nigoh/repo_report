PREFIX ?= /usr/local

.PHONY: install uninstall test test-tui

install:
	install -m0755 bin/repo-report $(PREFIX)/bin/repo-report

uninstall:
	rm -f $(PREFIX)/bin/repo-report

test:
	@bash -n bin/repo-report && echo "syntax OK"
	@bash tests/test_noninteractive.sh
	@bash tests/test_repo_detection.sh

test-tui:
	@echo "Running TUI key tests (requires a real TTY)..."
	@bash tests/test_tui_keys.sh
