# repo_report

`repo-report` is a single-file Bash CLI that walks a directory tree, finds
every nested git repository (both `.git` directories and `.git` gitfile
pointers — as used by Google's `repo` tool and submodules) and shows their
status. It has two faces:

- **Interactive TUI** (default on a real terminal) — an animated
  `🔴 LIVE · REPO REPORTER` news-ticker scrolls across the top while
  workers stream results into a scrollable list. Keys: `j/k` move,
  `/` filter, `f` fetch+rescan, `r` rescan, `q` quit.
- **Non-interactive** (when piped, or with `--format` / `-n`) — emits
  a `table` / `tsv` / `json` report, in parallel, suitable for pipelines,
  CI, and the `/repo-report` Claude Code skill.

It exists because the usual suspects (`gita`, `mr`, `ghq`, Google `repo`)
either require pre-registering repos or don't emit a compact machine-readable
report suitable for "is everything up-to-date?" checks across a `.repo`
workspace.

## Install

```sh
# copy / symlink onto your PATH
install -m0755 bin/repo-report /usr/local/bin/repo-report
# or
ln -s "$PWD/bin/repo-report" ~/.local/bin/repo-report
```

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
│ root:.  jobs:8  fetch:off  scanned:42/120  behind:3  dirty:1     │  ← status bar
├──────────────────────────────────────────────────────────────────┤
│ > workspace/proj-a        main  0a1b2c3  up-to-date  clean +0/-0 │  ← results
│   workspace/proj-b        main  3d4e5f6  behind      clean +0/-1 │
│   workspace/proj-c        main  7g8h9i0  up-to-date  dirty +0/-0 │
│   …                                                              │
├──────────────────────────────────────────────────────────────────┤
│ j/k move  / filter  f fetch+rescan  r rescan  q quit             │  ← help
╰──────────────────────────────────────────────────────────────────╯
```

The ticker is **data-driven** — every new `behind` / `ahead` / `diverged` /
`dirty` repository pushes a `⚡` or `⚠` item into the breaking-news strip
while the scan is still running.

**Keys**

| key          | action                              |
| ------------ | ----------------------------------- |
| `j` / ↓      | move cursor down                    |
| `k` / ↑      | move cursor up                      |
| `/`          | enter filter text, Enter to apply   |
| `f`          | toggle fetch flag and rescan        |
| `r`          | rescan (re-runs `find` + workers)   |
| `q` / Ctrl-C | quit (restores the terminal)        |

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

### Exit code

- `0` — every repo is `clean` and `up-to-date` (or has no upstream)
- `1` — at least one repo is dirty, behind, ahead, or diverged

### Parallelism

`repo-report` defaults to `nproc` workers and parallelises via `xargs -P`,
with NUL-delimited input so weird paths survive. Each worker emits a single
sub-`PIPE_BUF` line, so concurrent writes to stdout stay atomic on Linux.

Tune with `-j N`. For I/O-bound `--fetch` runs, setting `-j` well above
`nproc` (e.g. `-j 64`) is usually faster.

### Error codes

User-visible errors are emitted as `repo-report: [RRxxx] <message>` and
grouped by category: `RR1xx` argument parsing, `RR2xx` filesystem,
`RR3xx` git/worker, `RR4xx` TUI/terminal, `RR5xx` internal/deps.
See [`docs/errors.md`](docs/errors.md) for the full catalogue.

### Claude Code integration

This repo ships a sub-agent and a slash-command skill under `.claude/`:

- **`cli-reporter` agent** (`.claude/agents/cli-reporter.md`) —
  specialised for future Bash / TUI edits to this codebase.
- **`/repo-report` skill** (`.claude/skills/repo-report/SKILL.md`) —
  drives `bin/repo-report --non-interactive --format json` and
  summarises the result so Claude can answer "what's the state of
  my repos?" without eyeballing a 200-row table.
