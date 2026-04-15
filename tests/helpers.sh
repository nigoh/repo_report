#!/usr/bin/env bash
# tests/helpers.sh — shared utilities for repo-report tests
# Note: no set -e here; this file is sourced by test scripts that manage their own error handling

REPO_REPORT="${REPO_REPORT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/bin/repo-report}"

# strip_ansi: remove ANSI escape sequences from input
strip_ansi() { sed 's/\x1b\[[0-9;]*[mKHJGABCDsuhlr]//g; s/\x1b\[[0-9]*~//g'; }

# setup_fixture: create a minimal git repo in a temp dir
# Usage: setup_fixture
# Sets: FIXTURE_DIR
setup_fixture() {
  FIXTURE_DIR=$(mktemp -d)
  git -C "$FIXTURE_DIR" init -q
  git -C "$FIXTURE_DIR" config user.email "test@test"
  git -C "$FIXTURE_DIR" config user.name "Test"
  git -C "$FIXTURE_DIR" config gpg.format ""
  git -C "$FIXTURE_DIR" config commit.gpgsign false
  git -C "$FIXTURE_DIR" commit --allow-empty -m "init" -q
}

# setup_dirty_fixture: create a repo with a dirty file
setup_dirty_fixture() {
  setup_fixture
  touch "$FIXTURE_DIR/untracked.txt"
}

# setup_repo_ws_fixture: create a fixture with a .repo/ directory (AOSP-style)
setup_repo_ws_fixture() {
  setup_fixture
  mkdir -p "$FIXTURE_DIR/.repo"
}

# cleanup_fixture: remove the temp dir
cleanup_fixture() {
  [[ -n "${FIXTURE_DIR:-}" && -d "$FIXTURE_DIR" ]] && rm -rf "$FIXTURE_DIR"
}

# pass: print a green PASS message
pass() { printf '\e[32mPASS\e[0m  %s\n' "$1"; }

# fail: print a red FAIL message and exit 1
fail() { printf '\e[31mFAIL\e[0m  %s\n' "$1" >&2; FAILURES=$(( ${FAILURES:-0} + 1 )); }

# assert_eq: compare two values
assert_eq() {
  local label="$1" expected="$2" actual="$3"
  if [[ "$expected" == "$actual" ]]; then
    pass "$label"
  else
    fail "$label (expected='$expected' actual='$actual')"
  fi
}

# assert_contains: check that $actual contains $expected substring
assert_contains() {
  local label="$1" expected="$2" actual="$3"
  if [[ "$actual" == *"$expected"* ]]; then
    pass "$label"
  else
    fail "$label (expected to contain '$expected', got: '$actual')"
  fi
}
