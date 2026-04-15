# repo_report

[日本語版 README はこちら](README.ja.md)

![repo-report demo](demo.gif)

`repo-report` is a single-file Bash CLI that walks a directory tree, finds
every nested git repository (both `.git` directories and `.git` gitfile
pointers — as used by Google's `repo` tool and submodules) and shows their
status. It has two faces:

- **Interactive TUI** (default on a real terminal) — an animated
  `🔴 LIVE · REPO REPORTER` news-ticker scrolls across the top while
  workers stream results into a scrollable, filterable, sortable list.
  Press `?` for the full key-binding reference.
- **Non-interactive** (when piped, or with `--format` / `-n`) — emits
  a `table` / `tsv` / `json` report, in parallel, suitable for pipelines,
  CI, and the `/repo-report` Claude Code skill.

It exists because the usual suspects (`gita`, `mr`, `ghq`, Google `repo`)
either require pre-registering repos or don't emit a compact machine-readable
report suitable for "is everything up-to-date?" checks across a `.repo`
workspace.

## Usage without installing

After cloning, the script is immediately runnable — no install step needed:

```sh
git clone https://github.com/nigoh/repo_report.git
cd repo_report
./bin/repo-report /path/to/workspace
```

## Install (optional — adds `repo-report` to your PATH)

```sh
# via Makefile (installs to /usr/local/bin by default)
make install
# or to a custom prefix
make install PREFIX=~/.local

# manually
install -m0755 bin/repo-report /usr/local/bin/repo-report
# or with a symlink
ln -s "$PWD/bin/repo-report" ~/.local/bin/repo-report
```

To uninstall: `make uninstall`

Dependencies: `bash` (>=4), `git`, `find`, `xargs`, `awk`, `mkfifo`.
`column` is optional (used for table alignment in `--format table`).

## Interactive mode

Run with no arguments (or with a path) in a real terminal:

```sh
repo-report /path/to/workspace
```

Layout:

```
╭──────────────────────────────────────────────────────────────────╮
│ 🔴 LIVE · REPO REPORTER · scanned 42/120 · ⚡ 3 BEHIND · ⚠ 1 DIRTY │  ← scrolling ticker
├──────────────────────────────────────────────────────────────────┤
│ root:.  jobs:8  fetch:off  sort:path  scanned:42/120  behind:3   │  ← status bar
├▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓────────────────────────────────────┤  ← scan progress
│ > workspace/proj-a        main  0a1b2c3  up-to-date  clean +0/-0 │  ← results
│   workspace/proj-b        main  3d4e5f6  behind      clean +0/-1 │
│   workspace/proj-c        main  7g8h9i0  up-to-date  dirty  s:2  │  ← s:N = stash count
│   …                                                              │
├──────────────────────────────────────────────────────────────────┤
│ j/k/g/G move  PgUp/PgDn page  / filter  s sort  ? help  q quit   │  ← help bar
╰──────────────────────────────────────────────────────────────────╯
```

The ticker is **data-driven** — every new `behind` / `ahead` / `diverged` /
`dirty` repository pushes a `⚡` or `⚠` item into the breaking-news strip
while the scan is still running. Row 3 shows a **progress bar** during the
initial scan.

**Keys**

| key              | action                                              |
| ---------------- | --------------------------------------------------- |
| `j` / ↓          | move cursor down                                    |
| `k` / ↑          | move cursor up                                      |
| `g` / `G`        | jump to top / bottom of list                        |
| `PgDn` / `PgUp`  | scroll one page down / up                           |
| `/`              | live filter — type to narrow, Enter confirm, Esc cancel |
| `Esc`            | clear active filter                                 |
| `Enter`          | open detail pane for selected repo                  |
| `s`              | cycle sort mode: `path` → `status` → `date` → `branch` → `ahead-desc` → `behind-desc` |
| `f`              | toggle `--fetch` flag and rescan                    |
| `F`              | run `repo sync` (AOSP only)                         |
| `r`              | rescan (re-runs `find` + workers)                   |
| `d`              | diff overlay for selected repo (`git diff` + staged)|
| `T`              | `repo status` overlay (AOSP only)                   |
| `b`              | `repo branches` overlay (AOSP only)                 |
| `o`              | `repo overview` overlay (AOSP only)                 |
| `m`              | manifest XML overlay (AOSP only)                    |
| `n`              | `repo sync -n` — fetch only, no local update (AOSP only) |
| `B`              | `repo start <branch> --all` — start topic branch (AOSP only) |
| `A`              | `repo abandon <branch>` — delete topic branch, with confirmation (AOSP only) |
| `:`              | `repo forall -c <cmd>` — run command on all projects (AOSP only; destructive commands blocked) |
| `e`              | export current (filtered/sorted) view to a file     |
| `c`              | toggle column header row                            |
| `?`              | show full key-binding help overlay                  |
| `q` / Ctrl-C     | quit (restores the terminal)                        |

**Colour coding**

| colour | meaning       |
| ------ | ------------- |
| green  | up-to-date    |
| yellow | behind        |
| cyan   | ahead         |
| red    | diverged      |
| grey   | no upstream   |
| yellow `s:N` | stash entries present |

**AOSP / Google `repo` tool workspaces**

When the scan root contains a `.repo/` directory, `repo-report` detects it as
an AOSP workspace and enables additional keys:

| Key | Command | Notes |
| --- | ------- | ----- |
| `F` | `repo sync` | Full sync — destructive, use with care |
| `n` | `repo sync -n` | Fetch only — safe, no local update |
| `T` | `repo status` | All project status |
| `b` | `repo branches` | Branch list across all projects |
| `o` | `repo overview` | Recent commit summary |
| `m` | manifest XML | Contents of `.repo/manifest.xml` |
| `B` | `repo start <branch> --all` | Start topic branch on all projects |
| `A` | `repo abandon <branch>` | Delete topic branch (confirmation required) |
| `:` | `repo forall -c <cmd>` | Run arbitrary command on all projects; `reset --hard`, `clean -f`, `rm -rf` are blocked |

The current manifest branch is shown in the status bar as `repo:<branch>`.
These keys are silently ignored in non-AOSP workspaces.

## Non-interactive mode

Triggered by `--format`, `--non-interactive` / `-n`, or any non-TTY stdout:

```sh
# Explicit format
repo-report --format tsv  .  > report.tsv
repo-report --format json .  > report.json
repo-report --format table .

# Force non-interactive even from a TTY
repo-report -n /path/to/workspace

# Piped stdout auto-falls-back to TSV
repo-report . | awk -F'\t' 'NR>1 && $8=="behind"'

# Network refresh first (required for accurate behind/ahead counts)
repo-report -j 32 --fetch --format json /path/to/workspace > report.json

# CI gate: exit non-zero if anything is dirty / behind / ahead / diverged
repo-report --fetch -n . >/dev/null || echo "workspace not clean"
```

### Columns

| column    | meaning                                                 |
| --------- | ------------------------------------------------------- |
| `repo`    | path to the working tree                                |
| `branch`  | current branch (`(detached)` if HEAD is detached)       |
| `sha`     | short HEAD hash                                         |
| `date`    | HEAD commit date (ISO 8601)                             |
| `ahead`   | commits HEAD is ahead of `@{u}`                         |
| `behind`  | commits HEAD is behind `@{u}`                           |
| `dirty`   | `clean` or `dirty` (from `git status --porcelain`)      |
| `status`  | `up-to-date` / `behind` / `ahead` / `diverged` / `no-upstream` |
| `remote`  | `origin` URL                                            |
| `message` | HEAD commit subject                                     |
| `stash`   | number of stash entries (0 if none)                     |

### Exit code

- `0` — every repo is `clean` and `up-to-date` (or has no upstream)
- `1` — at least one repo is dirty, behind, ahead, or diverged

### Parallelism

`repo-report` defaults to `nproc` workers and parallelises via `xargs -P`,
with NUL-delimited input so weird paths survive. Each worker emits a single
sub-`PIPE_BUF` line, so concurrent writes to stdout stay atomic on Linux.

Tune with `-j N`. For I/O-bound `--fetch` runs, setting `-j` well above
`nproc` (e.g. `-j 64`) is usually faster.

### Scenarios

Place a `.repo-report.yml` file in the scan root to define **scenarios** — shell
commands that run automatically when events occur (sync complete, scan done, etc.)
or manually from within the TUI.

```yaml
scenarios:
  - name: "Build after sync"
    on: sync_done
    run: make -j8 build

  - name: "Alert on dirty repos"
    on: scan_done
    if: dirty_count > 0
    run: notify-send "Dirty repos found: $dirty_count"

  - name: "Full rebuild"
    on: manual
    run: make -j8 all
```

**Triggers** (`on:`):

| trigger | fired when |
| ------- | ---------- |
| `sync_done` | `repo sync` or `repo sync -n` completes (`F`/`n` keys) |
| `scan_done` | initial repo scan finishes |
| `manual` | user explicitly runs from scenario menu (`X` key) |

**Condition** (`if:`, optional) — evaluated before running; available variables:
`dirty_count`, `behind_count`, `ahead_count`, `diverged_count`, `total_count`.

**TUI keys for scenarios:**

| key | action |
| --- | ------ |
| `X` | open scenario selection menu |
| `L` | view output of last scenario run |

Results are announced in the ticker as `✓ NAME OK` or `✗ NAME FAILED`, and the
full stdout+stderr is accessible via `L`.

### Error codes

User-visible errors are emitted as `repo-report: [RRxxx] <message>` and
grouped by category: `RR1xx` argument parsing, `RR2xx` filesystem,
`RR3xx` git/worker, `RR4xx` TUI/terminal, `RR5xx` internal/deps.
See [`docs/errors.md`](docs/errors.md) for the full catalogue.

### Claude Code integration

This repo ships a sub-agent and a slash-command skill under `.claude/`:

- **`tui-reporter` agent** (`.claude/agents/tui-reporter.md`) —
  a pure-Bash + ANSI TUI specialist. Use it for any full-screen
  Bash tool under `bin/`, not just `repo-report`. It knows the
  alt-screen / raw-input / FIFO-streaming / news-ticker patterns
  and the invariants every TUI in this repo must preserve.
- **`/repo-report` skill** (`.claude/skills/repo-report/SKILL.md`) —
  a **TUI launcher**. Resolves `bin/repo-report` and a scan root,
  then hands you a ready-to-copy command that opens the interactive
  dashboard in your own terminal. Claude Code cannot drive a TUI,
  so the skill deliberately does not try to run it from the `Bash`
  tool. If you explicitly ask for "just the data" instead, it falls
  back to `--non-interactive --format json` and summarises.
