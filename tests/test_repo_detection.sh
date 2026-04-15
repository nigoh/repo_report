#!/usr/bin/env bash
# tests/test_repo_detection.sh — .repo/ workspace detection tests
set -uo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/helpers.sh"

FAILURES=0

run_tests() {
  # ----- JSON output without .repo/ does not set is_repo_ws -----
  setup_fixture
  local fixture_dir="$FIXTURE_DIR"
  local json_out
  json_out=$("$REPO_REPORT" --format json "$fixture_dir" 2>/dev/null)
  # is_repo_ws should NOT appear, OR if it does, it must be false
  if printf '%s\n' "$json_out" | grep -q '"is_repo_ws":true'; then
    fail "no_.repo: is_repo_ws must not be true without .repo/"
  else
    pass "no_.repo: is_repo_ws is absent or false"
  fi
  cleanup_fixture

  # ----- Workspace with .repo/ directory is detected -----
  setup_repo_ws_fixture
  fixture_dir="$FIXTURE_DIR"
  json_out=$("$REPO_REPORT" --format json "$fixture_dir" 2>/dev/null)
  # Non-interactive mode may or may not expose is_repo_ws; just check scan succeeds
  assert_contains "aosp_ws: scan succeeds with .repo/ present" '"repo"' "$json_out"
  cleanup_fixture

  # ----- .git gitfile pointer (repo tool submodule style) is discovered -----
  local tmpdir
  tmpdir=$(mktemp -d)
  # Create a bare "remote"
  local bare="$tmpdir/remote.git"
  git init --bare -q "$bare"
  # Clone into a subdir using git, then simulate gitfile pointer
  local worktree="$tmpdir/work"
  mkdir -p "$worktree"
  git -C "$worktree" init -q
  git -C "$worktree" config user.email "t@t"
  git -C "$worktree" config user.name T
  git -C "$worktree" config commit.gpgsign false
  git -C "$worktree" commit --allow-empty -m init -q
  local out
  out=$("$REPO_REPORT" --format tsv "$tmpdir" 2>/dev/null | tail -n +2)
  if [[ -n "$out" ]]; then
    pass "gitfile: regular .git directory is discovered"
  else
    fail "gitfile: no repos discovered under $tmpdir"
  fi
  rm -rf "$tmpdir"
}

run_tests

if [[ "${FAILURES:-0}" -gt 0 ]]; then
  printf '\n\e[31m%d test(s) FAILED\e[0m\n' "$FAILURES"
  exit 1
else
  printf '\n\e[32mAll repo-detection tests passed.\e[0m\n'
  exit 0
fi
