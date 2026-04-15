use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Gauge, List, ListItem, ListState, Paragraph, Wrap},
};

use crate::app::App;
use crate::types::{InputMode, Overlay, RepoStatus};

pub fn render(f: &mut Frame, app: &mut App) {
    let size = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // ticker
            Constraint::Length(1), // status bar
            Constraint::Length(1), // progress / separator
            Constraint::Min(3),    // list
            Constraint::Length(1), // help bar
        ])
        .split(size);

    render_ticker(f, app, chunks[0]);
    render_statusbar(f, app, chunks[1]);
    render_progress(f, app, chunks[2]);
    render_list(f, app, chunks[3]);
    render_helpbar(f, app, chunks[4]);

    // Overlays
    match app.overlay {
        Some(Overlay::Help) => render_help_overlay(f, app, size),
        Some(Overlay::Detail) => render_detail_overlay(f, app, size),
        Some(Overlay::Diff) => render_diff_overlay(f, app, size),
        Some(Overlay::AospCommand) | Some(Overlay::AospManifest) => {
            render_aosp_command_overlay(f, app, size)
        }
        Some(Overlay::AospConfirm) => render_aosp_confirm_overlay(f, app, size),
        Some(Overlay::AospPrompt) => render_aosp_prompt_overlay(f, app, size),
        None => {}
    }
}

// ─── Ticker ──────────────────────────────────────────────────────────────────

fn render_ticker(f: &mut Frame, app: &App, area: Rect) {
    let text = app.ticker_text();
    let doubled = format!("{text}{text}");
    let offset = app.ticker_offset % text.len().max(1);
    let display: String = doubled.chars().skip(offset).take(area.width as usize).collect();

    let para = Paragraph::new(display)
        .style(Style::default().bg(Color::DarkGray).fg(Color::White));
    f.render_widget(para, area);
}

// ─── Status bar ──────────────────────────────────────────────────────────────

fn render_statusbar(f: &mut Frame, app: &App, area: Rect) {
    let (total, dirty, behind, _) = app.counts();

    let filter_str = if app.filter.is_empty() {
        String::new()
    } else {
        format!("  filter:\"{}\"", app.filter)
    };
    let fetch_str = if app.fetch { " [fetch]" } else { "" };

    let aosp_str = if app.is_aosp {
        if app.manifest_branch.is_empty() {
            "  [AOSP]".to_string()
        } else {
            format!("  [AOSP] repo:{}", app.manifest_branch)
        }
    } else {
        String::new()
    };

    let text = format!(
        " {} repos | dirty:{} behind:{} | sort:{}{}{}{} ",
        total, dirty, behind,
        app.sort_mode.label(), fetch_str, filter_str, aosp_str
    );

    let para = Paragraph::new(text)
        .style(Style::default().bg(Color::Blue).fg(Color::White).add_modifier(Modifier::BOLD));
    f.render_widget(para, area);
}

// ─── Progress bar / separator ────────────────────────────────────────────────

fn render_progress(f: &mut Frame, app: &App, area: Rect) {
    if app.scanning && app.scan_total > 0 {
        let ratio = app.scan_done as f64 / app.scan_total as f64;
        let gauge = Gauge::default()
            .gauge_style(Style::default().fg(Color::Green).bg(Color::Black))
            .ratio(ratio)
            .label(format!("{}/{}", app.scan_done, app.scan_total));
        f.render_widget(gauge, area);
    } else {
        let sep = "─".repeat(area.width as usize);
        let para = Paragraph::new(sep)
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(para, area);
    }
}

// ─── Repo list ───────────────────────────────────────────────────────────────

fn render_list(f: &mut Frame, app: &mut App, area: Rect) {
    let visible = area.height as usize;
    app.clamp_scroll(visible);

    let mut items: Vec<ListItem> = Vec::new();

    if app.show_header {
        let header = Line::from(vec![Span::styled(
            format!(
                "{:<40} {:<12} {:<8} {:>5} {:>6} {:<8} {:<10}",
                "REPO", "BRANCH", "SHA", "AHEAD", "BEHIND", "DIRTY", "STATUS"
            ),
            Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )]);
        items.push(ListItem::new(header));
    }

    let start = app.scroll_offset;
    let end = (start + visible).min(app.filtered.len());

    for idx in start..end {
        let i = app.filtered[idx];
        let repo = &app.repos[i];
        let status_color = status_color(&repo.status);
        let dirty_color = if repo.dirty { Color::Red } else { Color::Green };

        let line = Line::from(vec![
            Span::raw(format!("{:<40} ", truncate(&repo.repo, 39))),
            Span::raw(format!("{:<12} ", truncate(&repo.branch, 11))),
            Span::raw(format!("{:<8} ", truncate(&repo.sha, 7))),
            Span::styled(format!("{:>5} ", repo.ahead), Style::default().fg(Color::Cyan)),
            Span::styled(format!("{:>6} ", repo.behind), Style::default().fg(Color::Yellow)),
            Span::styled(format!("{:<8} ", repo.dirty_str()), Style::default().fg(dirty_color)),
            Span::styled(format!("{:<10}", repo.status.as_str()), Style::default().fg(status_color)),
        ]);
        items.push(ListItem::new(line));
    }

    let mut list_state = ListState::default();
    if !app.filtered.is_empty() {
        let display_idx = app.selected - app.scroll_offset + if app.show_header { 1 } else { 0 };
        list_state.select(Some(display_idx));
    }

    let list = List::new(items)
        .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
        .highlight_symbol("> ");

    f.render_stateful_widget(list, area, &mut list_state);
}

// ─── Help bar ────────────────────────────────────────────────────────────────

fn render_helpbar(f: &mut Frame, app: &App, area: Rect) {
    let text = match &app.input_mode {
        InputMode::Filter => {
            format!(" Filter: {}█", app.filter)
        }
        InputMode::AospPrompt(op) => {
            let hint = op.prompt_hint().unwrap_or("input");
            format!(" {} > {}█", hint, app.aosp_prompt_buf)
        }
        InputMode::AospConfirm(op) => {
            format!(" Confirm: {} ? [y / N / Esc] ", op.label())
        }
        InputMode::Normal => {
            if app.is_aosp {
                " j/k:move  Enter:detail  d:diff  s:sort  f:fetch  r:rescan  /:filter  ?:help  q:quit \
                 | F:sync  n:sync-n  T:status  b:branches  m:manifest  D:download  M:make  C:clean  B:start  A:abandon  :::forall".to_string()
            } else {
                " j/k:move  Enter:detail  d:diff  s:sort  f:fetch  r:rescan  /:filter  ?:help  q:quit".to_string()
            }
        }
    };

    let para = Paragraph::new(text)
        .style(Style::default().bg(Color::DarkGray).fg(Color::White));
    f.render_widget(para, area);
}

// ─── Help overlay ────────────────────────────────────────────────────────────

fn render_help_overlay(f: &mut Frame, app: &App, area: Rect) {
    let popup = centered_rect(65, 85, area);
    f.render_widget(Clear, popup);

    let mut lines = vec![
        Line::from(Span::styled(
            "Key Bindings",
            Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )),
        Line::from(""),
        Line::from("  j / ↓       Move cursor down"),
        Line::from("  k / ↑       Move cursor up"),
        Line::from("  g           Jump to top"),
        Line::from("  G           Jump to bottom"),
        Line::from("  PgDn        Page down"),
        Line::from("  PgUp        Page up"),
        Line::from("  Enter       Open detail view"),
        Line::from("  d           Show git diff"),
        Line::from("  s           Cycle sort mode"),
        Line::from("  f           Toggle fetch & rescan"),
        Line::from("  r           Rescan"),
        Line::from("  /           Filter repos"),
        Line::from("  Esc         Clear filter / close overlay"),
        Line::from("  c           Toggle column header"),
        Line::from("  q           Quit"),
    ];

    if app.is_aosp {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  AOSP / repo tool",
            Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )));
        lines.push(Line::from("  F           repo sync -c -j{N} --no-tags"));
        lines.push(Line::from("  n           repo sync -n (fetch only)"));
        lines.push(Line::from("  T           repo status"));
        lines.push(Line::from("  b           repo branches"));
        lines.push(Line::from("  o           repo overview"));
        lines.push(Line::from("  m           manifest.xml viewer"));
        lines.push(Line::from("  D           repo download <project> <change>"));
        lines.push(Line::from("  M           make -j{N}"));
        lines.push(Line::from("  C           make clean  (confirmation required)"));
        lines.push(Line::from("  B           repo start <branch> --all"));
        lines.push(Line::from("  A           repo abandon <branch>  (confirmation required)"));
        lines.push(Line::from("  :           repo forall -c <cmd>"));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Press any key to close",
        Style::default().fg(Color::DarkGray),
    )));

    let block = Block::default()
        .title(" Help ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let para = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });
    f.render_widget(para, popup);
}

// ─── Detail overlay ──────────────────────────────────────────────────────────

fn render_detail_overlay(f: &mut Frame, app: &App, area: Rect) {
    let Some(repo) = app.selected_repo() else { return };
    let popup = centered_rect(70, 60, area);
    f.render_widget(Clear, popup);

    let status_color = status_color(&repo.status);
    let lines = vec![
        Line::from(vec![
            Span::styled("Repo:    ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(&repo.repo),
        ]),
        Line::from(vec![
            Span::styled("Branch:  ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(&repo.branch),
        ]),
        Line::from(vec![
            Span::styled("SHA:     ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(&repo.sha),
        ]),
        Line::from(vec![
            Span::styled("Date:    ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(&repo.date),
        ]),
        Line::from(vec![
            Span::styled("Status:  ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(repo.status.as_str(), Style::default().fg(status_color)),
        ]),
        Line::from(vec![
            Span::styled("Ahead:   ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(repo.ahead.to_string(), Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::styled("Behind:  ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(repo.behind.to_string(), Style::default().fg(Color::Yellow)),
        ]),
        Line::from(vec![
            Span::styled("Dirty:   ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(
                repo.dirty_str(),
                Style::default().fg(if repo.dirty { Color::Red } else { Color::Green }),
            ),
        ]),
        Line::from(vec![
            Span::styled("Stash:   ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(repo.stash.to_string()),
        ]),
        Line::from(vec![
            Span::styled("Remote:  ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(&repo.remote),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Message: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(&repo.message),
        ]),
        Line::from(""),
        Line::from(Span::styled("  Esc to close", Style::default().fg(Color::DarkGray))),
    ];

    let block = Block::default()
        .title(format!(" {} ", repo.repo))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let para = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });
    f.render_widget(para, popup);
}

// ─── Diff overlay ────────────────────────────────────────────────────────────

fn render_diff_overlay(f: &mut Frame, app: &App, area: Rect) {
    let popup = centered_rect(80, 80, area);
    f.render_widget(Clear, popup);

    let inner_h = popup.height.saturating_sub(2) as usize;
    let lines: Vec<Line> = app.diff_lines
        .iter()
        .skip(app.diff_scroll)
        .take(inner_h)
        .map(|l| {
            let color = if l.starts_with('+') { Color::Green }
                else if l.starts_with('-') { Color::Red }
                else if l.starts_with('@') { Color::Cyan }
                else { Color::Reset };
            Line::from(Span::styled(l.as_str(), Style::default().fg(color)))
        })
        .collect();

    let title = app.selected_repo()
        .map(|r| format!(" diff: {} ", r.repo))
        .unwrap_or_else(|| " diff ".to_string());

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));

    f.render_widget(Paragraph::new(lines).block(block), popup);
}

// ─── AOSP command output overlay ─────────────────────────────────────────────

fn render_aosp_command_overlay(f: &mut Frame, app: &App, area: Rect) {
    let popup = centered_rect(92, 88, area);
    f.render_widget(Clear, popup);

    let inner_h = popup.height.saturating_sub(2) as usize;

    let lines: Vec<Line> = app.aosp_output
        .iter()
        .skip(app.aosp_scroll)
        .take(inner_h)
        .map(|l| {
            let color = if l.starts_with("[err]") || l.to_lowercase().contains("error") || l.to_lowercase().contains("fatal") {
                Color::Red
            } else if l.starts_with('+') {
                Color::Green
            } else if l.to_lowercase().contains("warning") {
                Color::Yellow
            } else {
                Color::Reset
            };
            Line::from(Span::styled(l.as_str(), Style::default().fg(color)))
        })
        .collect();

    let spinner = ['|', '/', '-', '\\'];
    let spin = spinner[(app.ticker_tick as usize / 3) % 4];

    let title = match (&app.aosp_op, app.aosp_running, app.aosp_exit_ok) {
        (Some(op), true, _) => format!(" {} {} ", spin, op.label()),
        (Some(op), false, Some(true)) => format!(" ✓ {} ", op.label()),
        (Some(op), false, Some(false)) => format!(" ✗ {} ", op.label()),
        _ => " AOSP ".to_string(),
    };

    let border_color = match (app.aosp_running, app.aosp_exit_ok) {
        (true, _) => Color::Yellow,
        (false, Some(true)) => Color::Green,
        (false, Some(false)) => Color::Red,
        _ => Color::White,
    };

    let footer = if app.aosp_running {
        format!("  {} lines received — running…", app.aosp_output.len())
    } else {
        format!("  {} lines  |  j/k:scroll  Esc:close", app.aosp_output.len())
    };

    let block = Block::default()
        .title(title)
        .title_bottom(footer)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    f.render_widget(Paragraph::new(lines).block(block), popup);
}

// ─── AOSP confirmation overlay ───────────────────────────────────────────────

fn render_aosp_confirm_overlay(f: &mut Frame, app: &App, area: Rect) {
    let popup = centered_rect(52, 22, area);
    f.render_widget(Clear, popup);

    let op_label = app.aosp_op.as_ref()
        .or_else(|| {
            if let InputMode::AospConfirm(op) = &app.input_mode { Some(op) } else { None }
        })
        .map(|op| op.label())
        .unwrap_or("this operation");

    let lines = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            "  ⚠  Confirm destructive operation",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(op_label, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  This may delete or overwrite data.",
            Style::default().fg(Color::Red),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  [y] ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw("Confirm    "),
            Span::styled("[n / Esc] ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::raw("Cancel"),
        ]),
    ];

    let block = Block::default()
        .title(" Confirm ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    f.render_widget(Paragraph::new(lines).block(block), popup);
}

// ─── AOSP prompt overlay ─────────────────────────────────────────────────────

fn render_aosp_prompt_overlay(f: &mut Frame, app: &App, area: Rect) {
    // Render a 3-row box anchored to the bottom of the screen
    let popup = Rect {
        x: 0,
        y: area.height.saturating_sub(3),
        width: area.width,
        height: 3,
    };
    f.render_widget(Clear, popup);

    let hint = if let InputMode::AospPrompt(op) = &app.input_mode {
        op.prompt_hint().unwrap_or("Input")
    } else {
        "Input"
    };

    let line = Line::from(vec![
        Span::styled(format!(" {} > ", hint), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw(&app.aosp_prompt_buf),
        Span::styled("█", Style::default().fg(Color::White)),
        Span::styled("  [Enter] OK  [Esc] Cancel", Style::default().fg(Color::DarkGray)),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    f.render_widget(Paragraph::new(line).block(block), popup);
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn status_color(s: &RepoStatus) -> Color {
    match s {
        RepoStatus::UpToDate => Color::Green,
        RepoStatus::Behind => Color::Yellow,
        RepoStatus::Ahead => Color::Cyan,
        RepoStatus::Diverged => Color::Red,
        RepoStatus::NoUpstream => Color::DarkGray,
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max { s.to_string() }
    else { format!("{}…", &s[..max.saturating_sub(1)]) }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
