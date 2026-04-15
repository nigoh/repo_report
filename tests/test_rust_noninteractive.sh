#!/usr/bin/env bash
# tests/test_rust_noninteractive.sh
# Integration tests for the Ratatui/Rust binary (repo-report-tui)
# in non-interactive / format modes.

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
source "$SCRIPT_DIR/helpers.sh"

# Locate the binary (prefer release build, fall back to debug)
BINARY="${ROOT_DIR}/target/release/repo-report-tui"
if [[ ! -x "$BINARY" ]]; then
  BINARY="${ROOT_DIR}/target/debug/repo-report-tui"
fi
if [[ ! -x "$BINARY" ]]; then
  echo "ERROR: repo-report-tui binary not found. Run 'cargo build' first." >&2
  exit 1
fi

FAILURES=0

echo "=== Rust non-interactive tests: $BINARY ==="

# ── Helper ────────────────────────────────────────────────────────────────────

setup_git_fixture() {
  FIXTURE_DIR=$(mktemp -d)
  git -C "$FIXTURE_DIR" init -q -b main
  git -C "$FIXTURE_DIR" config user.email "test@test"
  git -C "$FIXTURE_DIR" config user.name "Test"
  git -C "$FIXTURE_DIR" config commit.gpgsign false
  echo "hello" > "$FIXTURE_DIR/hello.txt"
  git -C "$FIXTURE_DIR" add .
  git -C "$FIXTURE_DIR" commit -q -m "initial commit"
}

# ── TSV format ────────────────────────────────────────────────────────────────

echo
echo "--- TSV format ---"

setup_git_fixture
TSV_OUT=$("$BINARY" --format tsv "$FIXTURE_DIR" 2>&1)

# Header line
assert_contains "tsv: header contains 'repo'"     "repo"     "$TSV_OUT"
assert_contains "tsv: header contains 'branch'"   "branch"   "$TSV_OUT"
assert_contains "tsv: header contains 'status'"   "status"   "$TSV_OUT"
assert_contains "tsv: header contains 'dirty'"    "dirty"    "$TSV_OUT"
assert_contains "tsv: header contains 'stash'"    "stash"    "$TSV_OUT"

# Count exactly 11 tab-separated columns in the header
HEADER=$(echo "$TSV_OUT" | head -1)
COL_COUNT=$(echo "$HEADER" | awk -F'\t' '{print NF}')
assert_eq "tsv: header has 11 columns" "11" "$COL_COUNT"

# Data row exists
DATA_ROWS=$(echo "$TSV_OUT" | tail -n +2 | grep -v '^$' | wc -l | tr -d ' ')
assert_eq "tsv: has at least 1 data row" "1" "$([ "$DATA_ROWS" -ge 1 ] && echo 1 || echo 0)"

# Dirty field is 'clean' for a freshly committed repo
assert_contains "tsv: dirty field is 'clean'" "clean" "$TSV_OUT"

cleanup_fixture

# ── JSON format ───────────────────────────────────────────────────────────────

echo
echo "--- JSON format ---"

setup_git_fixture
JSON_OUT=$("$BINARY" --format json "$FIXTURE_DIR" 2>&1)

# Valid JSON array
assert_contains "json: output starts with '['" "[" "$(echo "$JSON_OUT" | head -1)"
assert_contains "json: output ends with ']'"   "]" "$(echo "$JSON_OUT" | tail -1)"

# Required fields
for field in repo branch sha date ahead behind dirty status remote message stash; do
  assert_contains "json: contains field '$field'" "\"$field\"" "$JSON_OUT"
done

# Parse with Python for structural validation
if command -v python3 >/dev/null 2>&1; then
  VALID=$(echo "$JSON_OUT" | python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    assert isinstance(data, list)
    if data:
        required = {'repo','branch','sha','date','ahead','behind','dirty','status','remote','message','stash'}
        assert required.issubset(data[0].keys()), f'missing keys: {required - data[0].keys()}'
    print('ok')
except Exception as e:
    print(f'FAIL: {e}')
" 2>&1)
  assert_eq "json: valid structure (python3)" "ok" "$VALID"
fi

cleanup_fixture

# ── TABLE format ──────────────────────────────────────────────────────────────

echo
echo "--- TABLE format ---"

setup_git_fixture
TABLE_OUT=$("$BINARY" --format table "$FIXTURE_DIR" 2>&1)

assert_contains "table: header has REPO"    "REPO"   "$TABLE_OUT"
assert_contains "table: header has BRANCH"  "BRANCH" "$TABLE_OUT"
assert_contains "table: header has STATUS"  "STATUS" "$TABLE_OUT"
assert_contains "table: has separator line" "─"      "$TABLE_OUT"

cleanup_fixture

# ── Exit codes ────────────────────────────────────────────────────────────────

echo
echo "--- Exit codes ---"

# Clean repo → exit 0
setup_git_fixture
"$BINARY" --format tsv "$FIXTURE_DIR" >/dev/null 2>&1
EC=$?
assert_eq "exit code 0 for clean repo" "0" "$EC"
cleanup_fixture

# Dirty repo → exit 1
setup_git_fixture
touch "$FIXTURE_DIR/untracked.txt"
"$BINARY" --format tsv "$FIXTURE_DIR" >/dev/null 2>&1
EC=$?
assert_eq "exit code 1 for dirty repo" "1" "$EC"
cleanup_fixture

# ── --max-depth flag ──────────────────────────────────────────────────────────

echo
echo "--- --max-depth flag ---"

OUTER=$(mktemp -d)
INNER="$OUTER/sub"
mkdir -p "$INNER"
git -C "$INNER" init -q -b main
git -C "$INNER" config user.email "t@t" && git -C "$INNER" config user.name "T"
git -C "$INNER" config commit.gpgsign false
git -C "$INNER" commit -q --allow-empty -m "init"

ROWS_DEEP=$("$BINARY" --format tsv --max-depth 3 "$OUTER" 2>&1 | tail -n +2 | grep -v '^$' | wc -l | tr -d ' ')
assert_eq "--max-depth 3 finds nested repo" "1" "$ROWS_DEEP"

ROWS_SHALLOW=$("$BINARY" --format tsv --max-depth 0 "$OUTER" 2>&1 | tail -n +2 | grep -v '^$' | wc -l | tr -d ' ')
assert_eq "--max-depth 0 skips nested repo" "0" "$ROWS_SHALLOW"

rm -rf "$OUTER"

# ── AOSP detection ────────────────────────────────────────────────────────────

echo
echo "--- AOSP detection ---"

setup_git_fixture
mkdir -p "$FIXTURE_DIR/.repo"
OUT=$("$BINARY" --format tsv "$FIXTURE_DIR" 2>&1)
# Even in non-interactive mode the scan should still work
assert_eq "aosp: tsv output still works with .repo present" "0" "$([ -n "$OUT" ] && echo 0 || echo 1)"
cleanup_fixture

# ── No repos scenario ─────────────────────────────────────────────────────────

echo
echo "--- Empty directory ---"

EMPTY=$(mktemp -d)
EMPTY_OUT=$("$BINARY" --format tsv "$EMPTY" 2>&1)
# Should output at least the header (or nothing — exit 0 either way)
"$BINARY" --format tsv "$EMPTY" >/dev/null 2>&1
EC=$?
assert_eq "empty dir: exit code 0" "0" "$EC"
rm -rf "$EMPTY"

# ── Summary ───────────────────────────────────────────────────────────────────

echo
if [[ "${FAILURES:-0}" -eq 0 ]]; then
  printf '\e[32mAll Rust integration tests passed.\e[0m\n'
  exit 0
else
  printf '\e[31m%d Rust integration test(s) FAILED.\e[0m\n' "$FAILURES" >&2
  exit 1
fi
