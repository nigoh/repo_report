---
name: repo-report
description: Run the local bin/repo-report CLI in non-interactive JSON mode against a workspace and summarise which repos are behind / ahead / diverged / dirty. Trigger when the user asks "what's the state of my repos", "are all my checkouts up to date", "show me the repo report", or types /repo-report. NEVER launch the interactive TUI from this skill — always pass --non-interactive --format json so output stays machine-parsable.
---

# /repo-report

Drives the repository's own `bin/repo-report` tool to produce a
data-driven status summary of every git checkout under a workspace,
without requiring the user to read a 200-row table by hand.

## Steps

1. **Resolve scan root**, in this order of precedence:
   1. An explicit path in the user's request
   2. `$REPO_REPORT_ROOT` environment variable
   3. A directory that contains a `.repo/` subdirectory (search from cwd upward one or two levels)
   4. `.` (current working directory)

2. **Locate the CLI.** Prefer in this order:
   1. `./bin/repo-report` (repo checkout)
   2. `repo-report` on `$PATH`

   If neither exists, stop and tell the user to install it with
   `install -m0755 bin/repo-report /usr/local/bin/` from the
   `nigoh/repo_report` checkout. Do not silently fall back to raw
   `git status` loops.

3. **Run the scan** with machine-readable output:

   ```bash
   <cli> --non-interactive --format json [--fetch] [-j N] <root>
   ```

   - Add `--fetch` **only** if the user explicitly asked for a network
     refresh (phrases like "latest", "after pull", "fetch first").
     Skip it for a local-only snapshot — `--fetch` can be slow across
     hundreds of repos.
   - Add `-j 32` (or similar) if the user complains about speed and
     the scan is I/O-bound against network.
   - Never pass `--interactive` or run the tool in a TTY-only mode
     from this skill — this skill's whole point is producing JSON
     that Claude can summarise.

4. **Parse the JSON.** Each element has keys:
   `repo, branch, sha, date, ahead, behind, dirty, status, remote, message`.

5. **Aggregate** into:
   - `total` — number of repos scanned
   - `up_to_date` — `status == "up-to-date"` and `dirty == "clean"`
   - `behind` — `status == "behind"`
   - `ahead` — `status == "ahead"`
   - `diverged` — `status == "diverged"`
   - `dirty` — `dirty == "dirty"`
   - `no_upstream` — `status == "no-upstream"`

6. **Report** in this shape (Markdown):

   ```
   ## Repo report for <root> (<total> repos)

   - ✅ up-to-date + clean: <n>
   - 🟡 behind: <n>
   - 🔵 ahead: <n>
   - 🔴 diverged: <n>
   - ⚠️  dirty: <n>
   - ⚫ no upstream: <n>
   ```

   Then a "needs attention" section listing **up to 20** worst offenders,
   prioritising in this order:
   1. `diverged` (most urgent — manual resolution)
   2. `dirty` (uncommitted work at risk)
   3. `behind >= 5`
   4. Everything else `behind`

   For each offender, show:
   `<repo>  <branch>  <sha>  <status>  +<ahead>/-<behind>  "<message>"`.

7. **Suggest next actions**, but **do not run them**:
   - For pure-behind repos: suggest `git -C <path> pull --ff-only`.
   - For diverged repos: suggest opening them for manual inspection;
     flag that rebase vs merge is a decision only the user should make.
   - For dirty repos: suggest `git -C <path> status` to see the changes.
   - If everything is clean + up-to-date, say so in a single sentence
     and stop — no action suggestions needed.

## Rules

- **Never mutate git state.** No pull, push, fetch (except via the
  `--fetch` flag on `repo-report` itself), reset, checkout, etc.
  This skill is a reporter, not an agent.
- **Never start the interactive TUI.** If you find yourself running
  `repo-report <path>` without `--non-interactive --format json`, you
  have made a mistake — Claude cannot drive a TUI.
- **Error-code awareness.** If the CLI exits with an `RRxxx` error
  on stderr, surface the code to the user and link to
  `docs/errors.md#RRxxx`.
- **Respect exit codes**: the CLI exits `1` if anything is dirty,
  behind, ahead, or diverged. Use this as a quick yes/no before
  diving into JSON parsing, but still produce a detailed summary so
  the user sees *what* is off.

## Example invocation

```bash
./bin/repo-report --non-interactive --format json --fetch /home/user/android
```

Parse the resulting JSON array, produce the Markdown summary above,
and stop. Do not offer to run git commands on the user's behalf
unless they explicitly ask.
