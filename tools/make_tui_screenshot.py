#!/usr/bin/env python3
"""Generate an SVG screenshot of the repo-report-tui interface.

Usage:
    python3 tools/make_tui_screenshot.py > demo-tui.svg
    python3 tools/make_tui_screenshot.py --aosp > demo-tui-aosp.svg
"""

import argparse
import html
import sys

# ── Palette (terminal-like) ───────────────────────────────────────────────────
BG       = "#1e1e2e"   # terminal background
FG       = "#cdd6f4"   # default foreground
TICKER   = "#585b70"   # ticker bar bg (DarkGray)
STATUS   = "#1e66f5"   # status bar bg (Blue)
HELPBAR  = "#45475a"   # help bar bg (DarkGray)
SEP      = "#6c7086"   # separator line
GREEN    = "#a6e3a1"
YELLOW   = "#f9e2af"
RED      = "#f38ba8"
CYAN     = "#89dceb"
GRAY     = "#6c7086"
WHITE    = "#cdd6f4"
MAGENTA  = "#cba6f7"
BOLDTEXT = "#ffffff"

# ── Layout constants ──────────────────────────────────────────────────────────
FONT     = "JetBrains Mono, Fira Code, Cascadia Code, Consolas, monospace"
FS       = 13          # font-size px
LH       = 20          # line height px
PAD_X    = 12          # horizontal padding
W        = 960         # total SVG width
TERM_H   = 380         # terminal area height

ROW_TICKER  = 0
ROW_STATUS  = 1
ROW_SEP     = 2
ROW_REPOS   = slice(3, 16)
ROW_HELP    = 16

TOTAL_ROWS = 18
H = TOTAL_ROWS * LH + 2 * 4   # +4px top/bottom

# ── Utility ───────────────────────────────────────────────────────────────────

def e(s): return html.escape(str(s))

def rect(x, y, w, h, fill, rx=0):
    return f'<rect x="{x}" y="{y}" width="{w}" height="{h}" fill="{fill}" rx="{rx}"/>'

def text(x, y, s, fill=FG, bold=False, mono=True):
    weight = 'bold' if bold else 'normal'
    family = FONT if mono else 'system-ui, sans-serif'
    return (f'<text x="{x}" y="{y}" font-family="{family}" '
            f'font-size="{FS}" fill="{fill}" font-weight="{weight}">{e(s)}</text>')

def row_y(row_index):
    """Top of a row (for rect), text baseline = top + LH - 5"""
    return 4 + row_index * LH

def text_y(row_index):
    return row_y(row_index) + LH - 5

def colored_text_spans(parts):
    """parts = list of (text, color). Returns SVG tspan elements."""
    spans = []
    for txt, color in parts:
        spans.append(f'<tspan fill="{color}">{e(txt)}</tspan>')
    return "".join(spans)

# ── Screen contents ───────────────────────────────────────────────────────────

def build_ticker(aosp: bool) -> str:
    if aosp:
        s = "  [AOSP] repo:android-14.0.0_r1  |  14 repos  dirty:1  behind:2  diverged:0  sort:path  "
    else:
        s = "  repo-report-tui  |  14 repos  dirty:1  behind:2  diverged:0  sort:path  root:/workspace  "
    # Double it and take width
    doubled = s + s
    display = doubled[:int(W / (FS * 0.6))]
    return display

def repo_rows(aosp: bool):
    """Return list of (col_parts, row_color) for the repo list body."""
    # col_parts: list of (text_segment, color)
    repos = [
        (".", "platform/frameworks/base", "main",    "a1b2c3d", "0", "0",  GREEN,  "clean", "up-to-date"),
        (".", "platform/frameworks/av",   "main",    "b2c3d4e", "0", "2",  YELLOW, "clean", "behind"),
        (".", "platform/system/core",     "main",    "c3d4e5f", "3", "0",  CYAN,   "clean", "ahead"),
        (".", "platform/packages/apps/Settings", "android-14", "d4e5f6a", "0", "0", GREEN, "clean", "up-to-date"),
        (".", "platform/external/curl",   "main",    "e5f6a7b", "1", "1",  RED,    "dirty", "diverged"),
        (".", "platform/build/soong",     "main",    "f6a7b8c", "0", "0",  GREEN,  "clean", "up-to-date"),
        (".", "platform/art",             "main",    "a7b8c9d", "0", "1",  YELLOW, "clean", "behind"),
        (".", "platform/bionic",          "main",    "b8c9d0e", "0", "0",  GREEN,  "clean", "up-to-date"),
        (".", "platform/hardware/interfaces", "main","c9d0e1f", "0", "0",  GRAY,   "clean", "no-upstream"),
        (".", "platform/kernel/configs",  "android-14","d0e1f2a","0","0",  GREEN,  "clean", "up-to-date"),
        (".", "platform/tools/apksig",    "main",    "e1f2a3b", "0", "0",  GREEN,  "clean", "up-to-date"),
        (".", "platform/libcore",         "main",    "f2a3b4c", "0", "0",  GREEN,  "clean", "up-to-date"),
        (".", "platform/packages/providers/MediaProvider", "main", "a3b4c5d", "0", "0", GREEN, "clean", "up-to-date"),
    ]
    rows = []
    for (root, repo, branch, sha, ahead, behind, s_color, dirty, status) in repos:
        dirty_c = RED if dirty == "dirty" else GREEN
        rows.append([
            (f"  {'> ' if repo == 'platform/frameworks/av' else '  '}{repo:<47}", WHITE if repo == 'platform/frameworks/av' else FG),
            (f" {branch:<14}", FG),
            (f" {sha:<9}", FG),
            (f" {ahead:>5}", CYAN),
            (f" {behind:>6}", YELLOW if int(behind) > 0 else FG),
            (f"  {dirty:<8}", dirty_c),
            (f" {status:<12}", s_color),
        ])
    return rows

# ── SVG builder ───────────────────────────────────────────────────────────────

def make_svg(aosp: bool) -> str:
    lines = []

    lines.append(f'''<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}"
     viewBox="0 0 {W} {H}" role="img" aria-label="repo-report-tui screenshot">
<defs>
  <style>
    text {{ dominant-baseline: auto; }}
  </style>
</defs>
''')

    # ── Background ───────────────────────────────────────────────────────────
    lines.append(rect(0, 0, W, H, BG))

    # ── Ticker row ───────────────────────────────────────────────────────────
    ty = row_y(ROW_TICKER)
    lines.append(rect(0, ty, W, LH, TICKER))
    ticker_txt = build_ticker(aosp)
    lines.append(text(PAD_X, text_y(ROW_TICKER), ticker_txt, fill=WHITE))

    # ── Status bar ───────────────────────────────────────────────────────────
    sy = row_y(ROW_STATUS)
    lines.append(rect(0, sy, W, LH, STATUS))
    if aosp:
        status_txt = "  14 repos | dirty:1 behind:2 | sort:path  [AOSP] repo:android-14.0.0_r1"
    else:
        status_txt = "  14 repos | dirty:1 behind:2 | sort:path"
    lines.append(text(PAD_X, text_y(ROW_STATUS), status_txt, fill=WHITE, bold=True))

    # ── Separator / progress bar ─────────────────────────────────────────────
    spy = row_y(ROW_SEP)
    sep_str = "─" * int(W / (FS * 0.55))
    lines.append(text(PAD_X, text_y(ROW_SEP), sep_str, fill=SEP))

    # ── Column header ────────────────────────────────────────────────────────
    header_row = 3
    hy = row_y(header_row)
    lines.append(rect(0, hy, W, LH, "#313244"))
    hdr = f"  {'REPO':<49} {'BRANCH':<15} {'SHA':<9} {'AHEAD':>6} {'BEHIND':>6}  {'DIRTY':<9} {'STATUS':<12}"
    lines.append(text(PAD_X, text_y(header_row), hdr, fill=WHITE, bold=True))

    # ── Repo rows ────────────────────────────────────────────────────────────
    repos = repo_rows(aosp)
    for i, parts in enumerate(repos):
        row_idx = 4 + i
        ry = row_y(row_idx)
        # Highlight selected row
        if i == 1:   # "behind" row highlighted
            lines.append(rect(0, ry, W, LH, "#313244"))
        # Build text with colored spans
        x = PAD_X
        base_y = text_y(row_idx)
        # Use a single text element with multiple tspans
        spans = colored_text_spans(parts)
        lines.append(
            f'<text x="{x}" y="{base_y}" font-family="{FONT}" '
            f'font-size="{FS}" font-weight="normal">{spans}</text>'
        )

    # ── Help bar ─────────────────────────────────────────────────────────────
    help_row = 4 + len(repos) + 1
    hly = row_y(help_row)
    lines.append(rect(0, hly, W, LH, HELPBAR))
    if aosp:
        help_txt = "  j/k:move  Enter:detail  d:diff  s:sort  /:filter  ?:help  q:quit  |  F:sync  n:sync-n  T:status  D:download  M:make  C:clean  B:start  A:abandon"
    else:
        help_txt = "  j/k:move  Enter:detail  d:diff  s:sort  f:fetch  r:rescan  /:filter  ?:help  q:quit"
    lines.append(text(PAD_X, text_y(help_row), help_txt, fill=WHITE))

    lines.append('</svg>')
    return "\n".join(lines)

# ── Main ─────────────────────────────────────────────────────────────────────

def main():
    parser = argparse.ArgumentParser(description="Generate repo-report-tui SVG screenshot")
    parser.add_argument("--aosp", action="store_true", help="Show AOSP workspace mode")
    parser.add_argument("-o", "--output", help="Output file (default: stdout)")
    args = parser.parse_args()

    svg = make_svg(args.aosp)

    if args.output:
        with open(args.output, "w") as f:
            f.write(svg)
        print(f"Written to {args.output}", file=sys.stderr)
    else:
        print(svg)

if __name__ == "__main__":
    main()
