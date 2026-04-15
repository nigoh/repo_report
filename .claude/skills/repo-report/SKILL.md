---
name: repo-report
description: TUI launcher for the repo-report interactive terminal dashboard. Resolve the bin/repo-report binary and a scan root, then hand the user a ready-to-copy command that launches the full-screen news-ticker TUI in their own terminal. Trigger on "launch repo-report", "show me the repo dashboard", "open the repo TUI", or explicit /repo-report. Claude CANNOT drive a TUI from this skill — do not execute the TUI command from the Bash tool. Only run the binary in --non-interactive --format json mode if the user explicitly asks for data instead of the TUI.
---

# /repo-report — TUI launcher

`bin/repo-report` is a pure-Bash + ANSI full-screen TUI with a scrolling
LIVE news-ticker reporter and streaming result list. Claude Code
**cannot drive a TUI** — the alt-screen protocol, raw input, and the
reporter animation all require a real user-facing terminal. So this
skill's job is to prepare everything the user needs and then **hand
off** a command line; it does not try to run the TUI on their behalf.

## Steps

### 1. Resolve the binary

Check, in order:

1. `./bin/repo-report` (repo checkout — this is the expected path
   when working inside `nigoh/repo_report`).
2. `repo-report` on `$PATH`.

If neither exists, print the install snippet and stop:

```sh
install -m0755 bin/repo-report /usr/local/bin/repo-report
# or, for a user install:
ln -s "$PWD/bin/repo-report" ~/.local/bin/repo-report
```

Do not silently fall back to raw `git status` loops — the skill's
value is the TUI, not reinventing it.

### 2. Resolve the scan root

Check, in order:

1. An explicit path in the user's message.
2. The `$REPO_REPORT_ROOT` environment variable.
3. A directory containing a `.repo/` subdirectory — search the
   current working directory and, if not found, its parent. This
   matches the common layout for Google `repo`-tool workspaces.
4. `.` (the current working directory).

### 3. Sanity-check the binary

Run `<cli> --version` (via the `Bash` tool) — it is fast, has no
TUI side effects, and only prints the version string. If it exits
non-zero, show the exact stderr and stop.

Do **not** run `<cli> --help` inside a pager; just read the output.
Do **not** run `<cli> <root>` without `--format` or `-n` — that
would try to launch the TUI from Claude's Bash sandbox, which will
either hang the tool call or corrupt the parent terminal.

### 4. Emit a ready-to-copy launch block

Print this exact shape to the user, with `<cli>` and `<root>`
substituted with the real values you resolved:

```
# Launch the repo-report TUI on <root>

    <cli> <root>

Keys:
    j / k / ↑ / ↓   move cursor
    /               filter repos (type query, Enter to apply)
    f               toggle fetch + rescan
    r               rescan
    q / Ctrl-C      quit (terminal is restored automatically)

If you want the data without opening the TUI:

    <cli> --non-interactive --format json <root>       # machine-readable
    <cli> --format tsv <root>                          # pipe into awk
    <cli> --format table <root>                        # pretty, non-interactive
```

Tell the user that the TUI needs to run in their own terminal — if
they are in a Claude Code session attached to a terminal, they can
paste the command directly; if they are in a web / headless session,
they will need to SSH in and run it there.

### 5. Do NOT execute the TUI command

Hard rule. The `Bash` tool runs commands in a sandbox whose stdout
is captured, not connected to the user's terminal. Running the TUI
command from Claude will:

- enter the alt screen inside the sandbox and never exit
- or print escape codes as literal text and corrupt Claude's output
- or leave `stty` in raw mode in the parent shell

**Only invoke `<cli>` from Bash with `--format <something>` or
`--non-interactive`.** Never bare.

### 6. Non-interactive fallback (opt-in only)

If the user explicitly says "I just want the data", "don't open the
TUI", "summarise", "what's the state of my repos", or similar, run:

```bash
<cli> --non-interactive --format json [--fetch] <root>
```

- Add `--fetch` only if they asked for a network refresh ("latest",
  "after pull", "check remotes"). It is slow on workspaces with
  hundreds of checkouts.
- Parse the JSON array. Each element has keys
  `repo, branch, sha, date, ahead, behind, dirty, status, remote, message`.
- Aggregate into counts: `total`, `up_to_date`
  (`status=="up-to-date" && dirty=="clean"`), `behind`, `ahead`,
  `diverged`, `dirty`, `no_upstream`.
- Produce a Markdown summary shaped like:

  ```
  ## Repo report for <root> (<total> repos)

  - ✅ up-to-date + clean: <n>
  - 🟡 behind: <n>
  - 🔵 ahead: <n>
  - 🔴 diverged: <n>
  - ⚠️  dirty: <n>
  - ⚫ no upstream: <n>
  ```

- Then a "needs attention" section of up to 20 worst offenders,
  prioritised in this order:
  1. `diverged` (manual resolution)
  2. `dirty` (uncommitted work at risk)
  3. `behind >= 5`
  4. Everything else `behind`

  For each offender show
  `<repo>  <branch>  <sha>  <status>  +<ahead>/-<behind>  "<message>"`.

- Suggest next actions but **do not run them**: `git -C <path> pull
  --ff-only` for pure-behind repos, manual inspection for diverged,
  `git -C <path> status` for dirty. If everything is clean + up-to-date,
  say so in one sentence and stop.

## Rules

- **Never mutate git state.** No pull, push, reset, checkout, etc.
  The only `git fetch` allowed is the one triggered by the CLI's own
  `--fetch` flag in the non-interactive fallback path.
- **Never launch the TUI from the `Bash` tool.** Only non-interactive
  invocations of `<cli>`.
- **Respect error codes.** If `<cli>` exits with `[RRxxx] …` on
  stderr, surface the code and link the user to
  `docs/errors.md#RRxxx` (e.g. `RR201` for a bad path, `RR203` for
  "no repos found", `RR402` for "terminal too small").
- **Don't second-guess the user's terminal.** If they say "I'm in
  tmux / iTerm / Kitty", trust them. If they say "I'm on the web
  app", tell them the TUI needs a real terminal.

## Example interaction

> User: /repo-report /home/user/android

Skill resolves `<cli>` = `./bin/repo-report`, `<root>` = `/home/user/android`,
runs `./bin/repo-report --version` to confirm it works, then replies:

```
# Launch the repo-report TUI on /home/user/android

    ./bin/repo-report /home/user/android

Keys:
    j / k / ↑ / ↓   move cursor
    /               filter repos (type query, Enter to apply)
    f               toggle fetch + rescan
    r               rescan
    q / Ctrl-C      quit (terminal is restored automatically)

If you want the data without opening the TUI:

    ./bin/repo-report --non-interactive --format json /home/user/android
    ./bin/repo-report --format tsv /home/user/android
    ./bin/repo-report --format table /home/user/android
```

Then stops.
