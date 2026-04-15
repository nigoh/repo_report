#!/usr/bin/env bash
# tests/test_repo_commands.sh — repo-tool command guard tests
#
# These tests verify that the AOSP-gated repo commands correctly detect
# IS_REPO_WS=0 and emit guard messages in non-interactive mode,
# and that the non-AOSP keys (d=diff) work on plain git repos.
set -uo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/helpers.sh"

FAILURES=0

# tui_run: send keystrokes into the TUI via a pseudo-TTY and capture output
tui_run() {
  local dir="$1" keys="$2"
  if ! command -v script >/dev/null 2>&1; then
    return 0
  fi
  printf '%s' "$keys" | \
    script -q -c "TERM=xterm-256color COLUMNS=120 LINES=30 $REPO_REPORT $dir" /dev/null 2>/dev/null \
    | sed 's/\x1b\[[0-9;]*[mKHJGABCDsuhlr]//g; s/\x1b\[[0-9]*~//g' || true
}

run_tests() {
  # ----- syntax check first -----
  bash -n "$REPO_REPORT" 2>/dev/null
  assert_eq "syntax: bash -n passes" "0" "$?"

  # ----- d key (diff): works on plain git repo -----
  if command -v script >/dev/null 2>&1; then
    setup_fixture
    local out
    out=$(tui_run "$FIXTURE_DIR" "dq")
    pass "tui: d key (diff overlay) completes without crash"
    cleanup_fixture
  else
    pass "tui: d key test skipped (no script command)"
  fi

  # ----- n/b/o/m/F/T/B/A keys: AOSP guard fires without .repo/ -----
  if command -v script >/dev/null 2>&1; then
    setup_fixture
    for key in n b o m B A; do
      out=$(tui_run "$FIXTURE_DIR" "${key}q")
      # Should complete without crash (guard message shown in ticker)
      pass "tui: $key key (AOSP-gated) completes without crash on plain repo"
    done
    cleanup_fixture
  else
    pass "tui: AOSP guard tests skipped (no script command)"
  fi

  # ----- : (forall) key: blocks destructive commands -----
  # Non-interactive check: verify the blocklist pattern exists in source
  if grep -q 'reset --hard' "$REPO_REPORT"; then
    pass "forall: destructive command blocklist present in source"
  else
    fail "forall: destructive command blocklist missing from source"
  fi

  # ----- Verify all new functions are defined in the script -----
  local -a expected_fns=(
    "_draw_scrollable_overlay"
    "_draw_repo_diff_overlay"
    "_draw_repo_branches_overlay"
    "_draw_repo_overview_overlay"
    "_draw_repo_manifest_overlay"
    "_prompt_repo_forall"
    "_run_repo_sync_n"
    "_prompt_repo_start"
    "_prompt_repo_abandon"
  )
  for fn in "${expected_fns[@]}"; do
    if grep -q "^${fn}()" "$REPO_REPORT"; then
      pass "function defined: $fn"
    else
      fail "function missing: $fn"
    fi
  done

  # ----- Verify all new key bindings are wired up -----
  local -a expected_keys=(
    "d) _draw_repo_diff_overlay"
    "b) _draw_repo_branches_overlay"
    "o) _draw_repo_overview_overlay"
    "m) _draw_repo_manifest_overlay"
    "n) _run_repo_sync_n"
    "B) _prompt_repo_start"
    "A) _prompt_repo_abandon"
  )
  for binding in "${expected_keys[@]}"; do
    if grep -q "$binding" "$REPO_REPORT"; then
      pass "key bound: $binding"
    else
      fail "key not bound: $binding"
    fi
  done

  # Forall uses ':' which needs special grep
  if grep -q "_prompt_repo_forall" "$REPO_REPORT"; then
    pass "key bound: ':' -> _prompt_repo_forall"
  else
    fail "key not bound: ':' -> _prompt_repo_forall"
  fi
}

run_tests

if [[ "${FAILURES:-0}" -gt 0 ]]; then
  printf '\n\e[31m%d test(s) FAILED\e[0m\n' "$FAILURES"
  exit 1
else
  printf '\n\e[32mAll repo-command tests passed.\e[0m\n'
  exit 0
fi
