# repo-report error codes

All user-visible errors from `bin/repo-report` are emitted as:

```
repo-report: [RRxxx] <message>
  -> see: https://github.com/nigoh/repo_report/blob/main/docs/errors.md#RRxxx
```

The `RR` prefix is short for "repo-report". Codes are grouped by category:

| range  | category                   |
| ------ | -------------------------- |
| RR1xx  | CLI / argument parsing     |
| RR2xx  | filesystem / discovery     |
| RR3xx  | git / worker               |
| RR4xx  | TUI / terminal             |
| RR5xx  | internal / dependencies    |

---

## RR1xx — CLI / arguments

### RR101
**Unknown option.**
An option starting with `-` was not recognised.
*Fix*: check `repo-report --help` for the supported flags. Exit code `2`.

### RR102
**`--format` value is invalid.**
Only `table`, `tsv`, and `json` are accepted.
*Fix*: `repo-report --format tsv ...`. Exit code `2`.

### RR103
**`--jobs` must be a positive integer.**
Values like `0`, negatives, or non-numeric strings are rejected.
*Fix*: `repo-report -j 8 ...`. Exit code `2`.

### RR104
**`--max-depth` / `--msg-width` must be a positive integer.**
Same rule as `--jobs`. Exit code `2`.

### RR105
**Flag conflict (reserved).**
Reserved for mutually-exclusive flag combinations added later. Exit code `2`.

---

## RR2xx — Filesystem / discovery

### RR201
**Path does not exist, or is not a directory.**
The positional `PATH` argument (or its default `.`) does not resolve to a directory.
*Fix*: point to the workspace root (e.g. the directory containing `.repo`). Exit code `1`.

### RR202
**Path is not readable.**
The directory exists but the current user cannot read it.
*Fix*: check permissions, or run with appropriate user. Exit code `1`.

### RR203
**No git repositories found under path.**
`find ... -name .git` returned nothing.
*Fix*: either you pointed at the wrong path, or nothing is checked out yet.
Note: this is **not** a hard failure — exit code is `0`, message goes to stderr,
so `repo-report | grep` pipelines still behave predictably on empty inputs.

---

## RR3xx — Git / worker

### RR301
**`git` binary not found on PATH.**
The script cannot run without it.
*Fix*: install git. Exit code `127`.

### RR302
**Worker failed to read a repository.**
Reserved for future use when a per-repo hard failure should bubble up.
In the current implementation, worker-level git failures are degraded
(the row gets `?`/`-` placeholders) rather than aborting the whole scan.

### RR303
**`git fetch` failed for one repo.**
Never fatal. In the TUI it appears as a non-fatal breaking-news ticker item:
`⚠ RR303 <repo>: fetch failed`. In non-interactive mode fetch errors are
silently ignored so the rest of the report still prints.

---

## RR4xx — TUI / terminal

### RR401
**Interactive mode requires a TTY.**
You tried to run the TUI (`repo-report` with no `--format` / no `-n`) but
stdin **and** stdout must both be a terminal. If stdout is piped, the
script auto-falls back to TSV and does not error; this code fires only
when a TTY was explicitly expected.
*Fix*: run from a real terminal, or pass `--format tsv` / `-n`. Exit `2`.

### RR402
**Terminal too small.**
The TUI needs at least **20 columns × 10 rows**.
*Fix*: resize the window, or use a non-interactive format. Exit code `2`.

### RR403
**`mkfifo` (or `mktemp -u`) failed.**
The TUI streams worker output through a named pipe; this code fires when
the pipe cannot be created (e.g. `$TMPDIR` is read-only).
*Fix*: set `TMPDIR` to a writable location. Exit code `1`.

### RR404
**Failed to enter or restore the alt screen.**
Reserved. Current code always restores in the `_cleanup` trap; if the
terminal is in a bad state after a crash, run `stty sane; printf '\e[?25h\e[?1049l'`.

---

## RR5xx — Internal / dependencies

### RR501
**Required dependency missing.**
One of `find`, `xargs`, `awk` was not found on PATH.
*Fix*: install the missing tool (typically part of `coreutils` / `findutils` /
`gawk` on Linux, or `findutils`+`gawk` via Homebrew on macOS). Exit code `127`.

### RR502
**Internal invariant violated.**
Please open an issue at https://github.com/nigoh/repo_report/issues with
the full command and terminal output. Exit code `70` (EX_SOFTWARE).
