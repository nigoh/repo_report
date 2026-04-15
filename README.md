# repo_report

`repo-report` is a single-file Bash CLI that walks a directory tree, finds
every nested git repository (both `.git` directories and `.git` gitfile
pointers — as used by Google's `repo` tool and submodules) and prints a
compact status report **in parallel**.

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

Dependencies: `bash` (>=4), `git`, `find`, `xargs`, `awk`. `column` is
optional (used for table alignment).

## Usage

```sh
# Scan current directory, one line per repo
repo-report

# Scan a workspace managed by the repo tool. Pass the workspace root,
# NOT .repo itself — the checkouts live alongside .repo.
repo-report /path/to/workspace

# Network refresh first (required for accurate behind/ahead counts)
repo-report -j 32 --fetch /path/to/workspace

# Machine-readable outputs
repo-report --format tsv  .  > report.tsv
repo-report --format json .  > report.json

# CI gate: exit non-zero if anything is dirty / behind / ahead / diverged
repo-report --fetch . >/dev/null || echo "workspace not clean"

# Find just the repos that fell behind upstream
repo-report --fetch --format tsv . | awk -F'\t' 'NR>1 && $8=="behind"'
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
