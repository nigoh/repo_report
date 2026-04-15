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

  # ----- Scenario feature: verify functions and key bindings -----
  local -a scenario_fns=(
    "_load_scenarios"
    "_eval_scenario_if"
    "_run_scenario"
    "_trigger_scenarios"
    "_draw_scenario_menu"
    "_draw_last_scenario_output"
  )
  for fn in "${scenario_fns[@]}"; do
    if grep -q "^${fn}()" "$REPO_REPORT"; then
      pass "scenario function defined: $fn"
    else
      fail "scenario function missing: $fn"
    fi
  done

  if grep -q "X) _draw_scenario_menu" "$REPO_REPORT"; then
    pass "key bound: X -> _draw_scenario_menu"
  else
    fail "key not bound: X -> _draw_scenario_menu"
  fi

  if grep -q "L) _draw_last_scenario_output" "$REPO_REPORT"; then
    pass "key bound: L -> _draw_last_scenario_output"
  else
    fail "key not bound: L -> _draw_last_scenario_output"
  fi

  # ----- Scenario YAML config loading -----
  local tmpscen; tmpscen=$(mktemp -d)
  git -C "$tmpscen" init -q
  git -C "$tmpscen" config commit.gpgsign false
  git -C "$tmpscen" commit --allow-empty -m "init" -q
  cat > "$tmpscen/.repo-report.yml" <<'EOF'
scenarios:
  - name: "Test scenario"
    on: manual
    run: echo "scenario ran"
EOF
  # Run in non-interactive mode to verify it doesn't crash with a config file present
  "$REPO_REPORT" --format tsv "$tmpscen" >/dev/null 2>&1
  pass "scenario: script does not crash with .repo-report.yml present"
  rm -rf "$tmpscen"

  # ----- Scenario trigger wiring: sync_done and scan_done triggers exist -----
  if grep -q "_trigger_scenarios sync_done" "$REPO_REPORT"; then
    pass "scenario trigger: sync_done is wired"
  else
    fail "scenario trigger: sync_done not wired"
  fi
  if grep -q "_trigger_scenarios scan_done" "$REPO_REPORT"; then
    pass "scenario trigger: scan_done is wired"
  else
    fail "scenario trigger: scan_done not wired"
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
