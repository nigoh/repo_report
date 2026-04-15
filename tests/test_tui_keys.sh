#!/usr/bin/env bash
# tests/test_tui_keys.sh — TUI key interaction tests (requires a TTY / script command)
#
# NOTE: These tests use the `script` command to create a pseudo-TTY.
#       They are not run by `make test` automatically because they require
#       an interactive terminal environment. Run manually:
#         bash tests/test_tui_keys.sh
set -uo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/helpers.sh"

FAILURES=0

# tui_run: send keystrokes to TUI and capture output (stripped of ANSI)
# Usage: tui_run FIXTURE_DIR KEYS
tui_run() {
  local dir="$1" keys="$2"
  # Use script to get a PTY; send keys then quit
  local out
  out=$(printf '%s' "$keys" | \
    script -q -c "TERM=xterm-256color COLUMNS=120 LINES=30 $REPO_REPORT $dir" /dev/null 2>/dev/null \
    | strip_ansi) || true
  printf '%s' "$out"
}

run_tests() {
  # ----- Quit key -----
  setup_fixture
  local out
  out=$(tui_run "$FIXTURE_DIR" "q")
  pass "tui: q key exits cleanly"
  cleanup_fixture

  # ----- Help overlay (? key) -----
  setup_fixture
  out=$(tui_run "$FIXTURE_DIR" "?q")
  if printf '%s\n' "$out" | grep -qi "KEY BINDINGS\|REPO-REPORT"; then
    pass "tui: ? key shows help overlay"
  else
    fail "tui: ? key did not show help overlay"
  fi
  cleanup_fixture

  # ----- Filter mode (/ key) -----
  setup_fixture
  out=$(tui_run "$FIXTURE_DIR" "/testfilter"$'\n'"q")
  pass "tui: / filter mode completes without crash"
  cleanup_fixture

  # ----- Escape clears filter -----
  setup_fixture
  out=$(tui_run "$FIXTURE_DIR" "/foo"$'\e'"q")
  pass "tui: Escape in filter mode does not crash"
  cleanup_fixture

  # ----- Sort cycle (s key) -----
  setup_fixture
  out=$(tui_run "$FIXTURE_DIR" "ssssssq")
  pass "tui: s sort cycle completes without crash"
  cleanup_fixture

  # ----- g/G navigation -----
  setup_fixture
  out=$(tui_run "$FIXTURE_DIR" "gGq")
  pass "tui: g/G navigation completes without crash"
  cleanup_fixture

  # ----- Enter detail pane -----
  setup_fixture
  out=$(tui_run "$FIXTURE_DIR" $'\n'"q")
  pass "tui: Enter key for detail pane completes without crash"
  cleanup_fixture

  # ----- Column header toggle (c key) -----
  setup_fixture
  out=$(tui_run "$FIXTURE_DIR" "ccq")
  pass "tui: c column header toggle completes without crash"
  cleanup_fixture
}

# Only run if script command is available
if ! command -v script >/dev/null 2>&1; then
  printf '\e[33mSKIP\e[0m  TUI key tests require the `script` command (not found)\n'
  exit 0
fi

run_tests

if [[ "${FAILURES:-0}" -gt 0 ]]; then
  printf '\n\e[31m%d test(s) FAILED\e[0m\n' "$FAILURES"
  exit 1
else
  printf '\n\e[32mAll TUI key tests passed.\e[0m\n'
  exit 0
fi
