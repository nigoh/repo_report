#!/usr/bin/env bash
# tests/test_noninteractive.sh — non-interactive mode output validation
set -uo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/helpers.sh"

FAILURES=0

run_tests() {
  local fixture_dir

  # ----- TSV format -----
  setup_fixture
  fixture_dir="$FIXTURE_DIR"

  local tsv_out
  tsv_out=$("$REPO_REPORT" --format tsv "$fixture_dir" 2>/dev/null)

  # Header line exists
  assert_contains "tsv: header contains 'repo'" "repo	branch" "$tsv_out"

  # At least one data line (NF >= 10)
  local data_lines
  data_lines=$(printf '%s\n' "$tsv_out" | tail -n +2 | awk -F'\t' 'NF>=10{c++} END{print c+0}')
  assert_eq "tsv: data lines >= 1" "1" "$data_lines"

  cleanup_fixture

  # ----- JSON format -----
  setup_fixture
  fixture_dir="$FIXTURE_DIR"

  local json_out
  json_out=$("$REPO_REPORT" --format json "$fixture_dir" 2>/dev/null)

  assert_contains "json: is array" "[" "$json_out"
  assert_contains "json: has repo field" '"repo"' "$json_out"
  assert_contains "json: has branch field" '"branch"' "$json_out"
  assert_contains "json: has status field" '"status"' "$json_out"

  cleanup_fixture

  # ----- TABLE format -----
  setup_fixture
  fixture_dir="$FIXTURE_DIR"

  local table_out
  table_out=$("$REPO_REPORT" --format table "$fixture_dir" 2>/dev/null)

  assert_contains "table: has REPO header" "REPO" "$table_out"
  assert_contains "table: has STATUS header" "STATUS" "$table_out"

  cleanup_fixture

  # ----- exit code: clean repo = 0 -----
  setup_fixture
  fixture_dir="$FIXTURE_DIR"

  "$REPO_REPORT" -n "$fixture_dir" >/dev/null 2>&1
  local ec=$?
  assert_eq "exit_code: clean repo exits 0" "0" "$ec"

  cleanup_fixture

  # ----- exit code: dirty repo = 1 -----
  setup_dirty_fixture
  fixture_dir="$FIXTURE_DIR"

  "$REPO_REPORT" -n "$fixture_dir" >/dev/null 2>&1; ec=$?
  assert_eq "exit_code: dirty repo exits 1" "1" "$ec"

  cleanup_fixture

  # ----- stash column (field 11) -----
  setup_fixture
  fixture_dir="$FIXTURE_DIR"

  local tsv_hdr
  tsv_hdr=$("$REPO_REPORT" --format tsv "$fixture_dir" 2>/dev/null | head -1)
  assert_contains "tsv: header has stash field" "stash" "$tsv_hdr"

  local json_stash
  json_stash=$("$REPO_REPORT" --format json "$fixture_dir" 2>/dev/null)
  assert_contains "json: has stash field" '"stash"' "$json_stash"

  cleanup_fixture

  # ----- max-depth option -----
  local tmproot
  tmproot=$(mktemp -d)
  mkdir -p "$tmproot/deep/nested"
  (cd "$tmproot/deep/nested" && git init -q && git config user.email "t@t" && git config user.name T && git config commit.gpgsign false && git commit --allow-empty -m init -q)

  # With --max-depth 1 the nested repo is too deep: no repos found → no output
  local depth1_out
  depth1_out=$("$REPO_REPORT" --format tsv --max-depth 1 "$tmproot" 2>/dev/null | tail -n +2 | wc -l | tr -d ' ')
  assert_eq "max-depth: depth 1 finds 0 data rows" "0" "$depth1_out"

  rm -rf "$tmproot"

  # ----- no repos found (exit 0) -----
  local emptydir
  emptydir=$(mktemp -d)
  "$REPO_REPORT" -n "$emptydir" >/dev/null 2>&1
  ec=$?
  assert_eq "no repos: exits 0" "0" "$ec"
  rm -rf "$emptydir"

  # ----- syntax check -----
  bash -n "$REPO_REPORT" 2>/dev/null
  ec=$?
  assert_eq "syntax: bash -n passes" "0" "$ec"
}

run_tests

if [[ "${FAILURES:-0}" -gt 0 ]]; then
  printf '\n\e[31m%d test(s) FAILED\e[0m\n' "$FAILURES"
  exit 1
else
  printf '\n\e[32mAll non-interactive tests passed.\e[0m\n'
  exit 0
fi
