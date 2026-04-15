PREFIX ?= /usr/local

.PHONY: install uninstall

install:
	install -m0755 bin/repo-report $(PREFIX)/bin/repo-report

uninstall:
	rm -f $(PREFIX)/bin/repo-report
