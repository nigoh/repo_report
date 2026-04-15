#!/usr/bin/env python3
"""
tools/make_demo_gif.py — Generate an animated GIF demo of repo-report TUI.

Creates a synthetic fixture workspace with repos in various states, then
renders a sequence of terminal "frames" as an animated GIF using PIL.

Usage:
  python3 tools/make_demo_gif.py [output.gif]
"""

import os
import sys
import subprocess
import tempfile
import shutil
import textwrap
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

# ── configuration ──────────────────────────────────────────────────────────────
OUT_FILE   = sys.argv[1] if len(sys.argv) > 1 else "demo.gif"
COLS, ROWS = 110, 32
FONT_SIZE  = 14
FONT_PATH  = "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf"
FONT_BOLD  = "/usr/share/fonts/truetype/dejavu/DejaVuSansMono-Bold.ttf"
FRAME_MS   = 120   # default frame delay in ms
LOOP       = 0     # 0 = loop forever

# ANSI-like terminal colours (dark theme)
BG      = (18,  18,  18)
FG      = (204, 204, 204)
GREEN   = (80,  200, 80)
YELLOW  = (220, 180, 40)
CYAN    = (80,  200, 220)
RED     = (220, 70,  70)
GREY    = (120, 120, 120)
MAGENTA = (200, 80,  200)
WHITE   = (240, 240, 240)
REV_BG  = (204, 204, 204)  # reversed (ticker/header) background
REV_FG  = (18,  18,  18)   # reversed foreground

# ── font setup ─────────────────────────────────────────────────────────────────
def load_font(path, size):
    try:
        return ImageFont.truetype(path, size)
    except Exception:
        return ImageFont.load_default()

font      = load_font(FONT_PATH, FONT_SIZE)
font_bold = load_font(FONT_BOLD, FONT_SIZE)

# Measure cell size
_test_img = Image.new("RGB", (200, 40), BG)
_d = ImageDraw.Draw(_test_img)
bbox = _d.textbbox((0, 0), "M", font=font)
CELL_W = bbox[2] - bbox[0] + 1
CELL_H = bbox[3] - bbox[1] + 4

IMG_W = COLS * CELL_W
IMG_H = ROWS * CELL_H

# ── primitive drawing ───────────────────────────────────────────────────────────
def new_frame():
    img = Image.new("RGB", (IMG_W, IMG_H), BG)
    return img, ImageDraw.Draw(img)

def put_text(draw, row, col, text, fg=FG, bg=None, bold=False):
    x = col * CELL_W
    y = row * CELL_H
    f = font_bold if bold else font
    if bg:
        draw.rectangle([x, y, x + len(text) * CELL_W, y + CELL_H], fill=bg)
    draw.text((x, y), text, fill=fg, font=f)

def fill_row(draw, row, bg):
    draw.rectangle([0, row * CELL_H, IMG_W, (row + 1) * CELL_H], fill=bg)

def hline(draw, row, char="-", fg=GREY):
    put_text(draw, row, 0, char * COLS, fg=fg)

# ── fixture workspace creation ─────────────────────────────────────────────────
def run(cmd, cwd=None, check=True):
    return subprocess.run(
        cmd, shell=True, cwd=cwd, capture_output=True, text=True,
        check=check
    )

def git(repo_dir, cmd):
    return run(f"git -C {repo_dir} {cmd}")

def make_fixture():
    tmpdir = tempfile.mkdtemp(prefix="repo-report-demo-")

    def make_repo(name, state):
        """state: clean|behind|ahead|dirty|diverged|no-upstream"""
        d = os.path.join(tmpdir, name)
        os.makedirs(d)
        git(d, "init -q")
        git(d, 'config user.email "demo@demo"')
        git(d, 'config user.name "Demo"')
        git(d, "config commit.gpgsign false")

        if state == "no-upstream":
            git(d, "commit --allow-empty -m 'init' -q")
            return d

        # set up a bare remote
        bare = d + ".git"
        run(f"git init --bare -q {bare}")
        git(d, f"remote add origin {bare}")
        git(d, "commit --allow-empty -m 'base' -q")
        git(d, "push -u origin HEAD:main -q")

        if state == "clean":
            pass
        elif state == "behind":
            # add a commit to remote
            clone = tempfile.mkdtemp()
            run(f"git clone -q {bare} {clone}")
            run(f"git -C {clone} config user.email x@x")
            run(f"git -C {clone} config user.name X")
            run(f"git -C {clone} config commit.gpgsign false")
            run(f"git -C {clone} commit --allow-empty -m 'upstream commit' -q")
            run(f"git -C {clone} push -q")
            shutil.rmtree(clone)
            git(d, "fetch -q")
        elif state == "ahead":
            git(d, "commit --allow-empty -m 'local commit' -q")
        elif state == "dirty":
            Path(d, "change.txt").write_text("dirty\n")
        elif state == "diverged":
            # ahead
            git(d, "commit --allow-empty -m 'local diverge' -q")
            # behind
            clone = tempfile.mkdtemp()
            run(f"git clone -q {bare} {clone}")
            run(f"git -C {clone} config user.email x@x")
            run(f"git -C {clone} config user.name X")
            run(f"git -C {clone} config commit.gpgsign false")
            run(f"git -C {clone} commit --allow-empty -m 'remote diverge' -q")
            run(f"git -C {clone} push -q")
            shutil.rmtree(clone)
            git(d, "fetch -q")
        return d

    repos = [
        ("platform/frameworks/base",    "clean"),
        ("platform/system/core",        "behind"),
        ("platform/hardware/interfaces","clean"),
        ("vendor/app/feature",          "dirty"),
        ("vendor/app/analytics",        "ahead"),
        ("external/libfoo",             "behind"),
        ("external/libbar",             "diverged"),
        ("tools/build",                 "clean"),
        ("tools/test",                  "no-upstream"),
    ]

    for name, state in repos:
        os.makedirs(os.path.join(tmpdir, os.path.dirname(name)), exist_ok=True)
        make_repo(name, state)

    return tmpdir

# ── collect real data from repo-report ─────────────────────────────────────────
def collect_data(root):
    script = str(Path(__file__).parent.parent / "bin" / "repo-report")
    result = run(f"{script} --format tsv {root}", check=False)
    lines = result.stdout.strip().split("\n")
    rows = []
    for line in lines[1:]:   # skip header
        if not line.strip():
            continue
        parts = line.split("\t")
        if len(parts) >= 8:
            rows.append({
                "repo":    parts[0],
                "branch":  parts[1],
                "sha":     parts[2],
                "ahead":   int(parts[4]) if parts[4].isdigit() else 0,
                "behind":  int(parts[5]) if parts[5].isdigit() else 0,
                "dirty":   parts[6],
                "status":  parts[7],
                "stash":   int(parts[10]) if len(parts) > 10 and parts[10].strip().isdigit() else 0,
            })
    return rows

STATUS_COLOR = {
    "up-to-date": GREEN,
    "behind":     YELLOW,
    "ahead":      CYAN,
    "diverged":   RED,
    "no-upstream":GREY,
    "dirty":      RED,
}

def status_color(status, dirty):
    if dirty == "dirty":
        return RED
    return STATUS_COLOR.get(status, FG)

def short_repo(name, maxlen):
    if len(name) <= maxlen:
        return name.ljust(maxlen)
    return ("…" + name[-(maxlen-1):]).ljust(maxlen)

# ── frame builders ─────────────────────────────────────────────────────────────
_TICKER_BASE = (
    "   [*] LIVE  *  REPO REPORTER  *  scanned {sc}/{tot}"
    "  *  >> {bh} BEHIND  *  >> 1 DIVERGED"
    "  *  >> {ah} AHEAD  *  !! {dy} DIRTY"
    "  *  press ? for help, q to quit   "
    "                              "
)

def make_ticker(scanned, total, behind, ahead, dirty):
    return _TICKER_BASE.format(sc=scanned, tot=total, bh=behind, ah=ahead, dy=dirty)

def draw_frame(rows, ticker_offset=0, selected=0, scroll=0, scanned=None, total=None, label="",
               n_behind=2, n_ahead=1, n_dirty=1):
    img, draw = new_frame()

    if scanned is None: scanned = len(rows)
    if total   is None: total   = len(rows)

    # Row 0: ticker (scrolling)
    ticker = make_ticker(scanned, total, n_behind, n_ahead, n_dirty)
    doubled = ticker + ticker
    off = ticker_offset % len(ticker)
    slice_t = doubled[off: off + COLS]
    fill_row(draw, 0, REV_BG)
    put_text(draw, 0, 0, slice_t[:COLS].ljust(COLS), fg=REV_FG, bg=REV_BG, bold=True)

    # Row 1: status bar
    status_bar = (
        f"root:{label}  jobs:8  fetch:off  sort:path  "
        f"scanned:{scanned}/{total}  behind:2  ahead:1  dirty:1  diverged:1  filter:-"
    )
    put_text(draw, 1, 0, status_bar[:COLS].ljust(COLS), fg=GREY)

    # Row 2: separator / progress
    if scanned < total:
        fill = (COLS * scanned) // total
        seg1 = "▓" * fill
        seg2 = "-" * (COLS - fill)
        put_text(draw, 2, 0, seg1, fg=GREEN)
        put_text(draw, 2, fill, seg2, fg=GREY)
    else:
        hline(draw, 2)

    # Rows 3..ROWS-3: repo list
    visible = ROWS - 5
    top = 3
    MAXREPO = COLS - 55
    if MAXREPO < 20: MAXREPO = 20

    for i in range(visible):
        screen_row = top + i
        idx = scroll + i
        if idx >= len(rows):
            break
        r = rows[idx]

        repo_s   = short_repo(r["repo"], MAXREPO)
        branch_s = r["branch"][:14].ljust(14)
        sha_s    = r["sha"][:8].ljust(8)
        status_s = r["status"][:11].ljust(11)
        dirty_s  = r["dirty"][:5].ljust(5)
        ah_bh    = f"+{r['ahead']}/-{r['behind']}"
        stash_s  = f" s:{r['stash']}" if r["stash"] > 0 else ""

        col = status_color(r["status"], r["dirty"])

        # cursor highlight
        is_sel = (idx == selected)
        row_bg = (40, 40, 60) if is_sel else BG

        draw.rectangle([0, screen_row * CELL_H, IMG_W, (screen_row+1)*CELL_H], fill=row_bg)
        marker = "> " if is_sel else "  "
        put_text(draw, screen_row, 0,           marker,   fg=WHITE,  bg=row_bg, bold=is_sel)
        put_text(draw, screen_row, 2,           repo_s,   fg=FG,     bg=row_bg)
        put_text(draw, screen_row, 2+MAXREPO+2, branch_s, fg=GREY,   bg=row_bg)
        put_text(draw, screen_row, 2+MAXREPO+18,sha_s,    fg=GREY,   bg=row_bg)
        put_text(draw, screen_row, 2+MAXREPO+28,status_s, fg=col,    bg=row_bg, bold=(r["status"]!="up-to-date"))
        put_text(draw, screen_row, 2+MAXREPO+41,dirty_s,  fg=(RED if r["dirty"]=="dirty" else GREEN), bg=row_bg)
        put_text(draw, screen_row, 2+MAXREPO+48,ah_bh,    fg=GREY,   bg=row_bg)
        if stash_s:
            put_text(draw, screen_row, 2+MAXREPO+56, stash_s, fg=YELLOW, bg=row_bg)

    # Row ROWS-2: separator
    hline(draw, ROWS - 2)

    # Row ROWS-1: help bar
    help_text = " j/k/g/G move  PgUp/PgDn page  / filter  s sort  d diff  X scenario  f fetch  r rescan  ? help  q quit "
    put_text(draw, ROWS - 1, 0, help_text[:COLS].ljust(COLS), fg=GREY)

    return img

# ── animation sequence ─────────────────────────────────────────────────────────
def build_frames(rows):
    frames, delays = [], []

    # Count problem repos for ticker stats
    n_behind  = sum(1 for r in rows if r["status"] == "behind")
    n_ahead   = sum(1 for r in rows if r["status"] == "ahead")
    n_dirty   = sum(1 for r in rows if r["dirty"] == "dirty")

    # Phase 1: scanning (6 frames, progressively more repos)
    n = len(rows)
    for step in range(0, n + 1, max(1, n // 5)):
        partial = rows[:step] if step > 0 else []
        # Count partial stats as they accumulate
        pb = sum(1 for r in partial if r["status"] == "behind")
        pa = sum(1 for r in partial if r["status"] == "ahead")
        pd = sum(1 for r in partial if r["dirty"] == "dirty")
        img = draw_frame(partial, ticker_offset=step * 3, selected=0,
                         scanned=step, total=n, label="workspace/",
                         n_behind=pb, n_ahead=pa, n_dirty=pd)
        frames.append(img)
        delays.append(FRAME_MS * 2)

    # Phase 2: full list, ticker scrolling, cursor at 0
    for t in range(12):
        img = draw_frame(rows, ticker_offset=t * 4, selected=0,
                         scanned=n, total=n, label="workspace/",
                         n_behind=n_behind, n_ahead=n_ahead, n_dirty=n_dirty)
        frames.append(img)
        delays.append(FRAME_MS)

    # Phase 3: cursor moves down through problem repos
    for sel in range(0, min(7, n)):
        img = draw_frame(rows, ticker_offset=sel * 5 + 50, selected=sel,
                         scanned=n, total=n, label="workspace/",
                         n_behind=n_behind, n_ahead=n_ahead, n_dirty=n_dirty)
        frames.append(img)
        delays.append(FRAME_MS * 3)

    # Phase 4: hold on a behind repo
    for t in range(6):
        img = draw_frame(rows, ticker_offset=t * 3 + 90, selected=1,
                         scanned=n, total=n, label="workspace/",
                         n_behind=n_behind, n_ahead=n_ahead, n_dirty=n_dirty)
        frames.append(img)
        delays.append(FRAME_MS)

    return frames, delays

# ── main ───────────────────────────────────────────────────────────────────────
def main():
    print("Creating fixture workspace…")
    tmpdir = make_fixture()

    try:
        print("Running repo-report to collect data…")
        rows = collect_data(tmpdir)
        if not rows:
            print("ERROR: no data collected. Is bin/repo-report available?", file=sys.stderr)
            sys.exit(1)
        print(f"  Found {len(rows)} repos")

        print("Building animation frames…")
        frames, delays = build_frames(rows)

        print(f"Saving {OUT_FILE}…")
        frames[0].save(
            OUT_FILE,
            save_all=True,
            append_images=frames[1:],
            duration=delays,
            loop=LOOP,
            optimize=False,
        )
        print(f"Done → {OUT_FILE}  ({len(frames)} frames, {os.path.getsize(OUT_FILE)//1024} KB)")

    finally:
        shutil.rmtree(tmpdir, ignore_errors=True)

if __name__ == "__main__":
    main()
