use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Gauge, List, ListItem, ListState, Paragraph, Wrap},
};
use crate::app::App;
use crate::types::{Overlay, RepoStatus};

pub fn render(f: &mut Frame, app: &mut App) {
    let size = f.area();

    // Layout: ticker | statusbar | list | helpbar
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
        Some(Overlay::Help) => render_help_overlay(f, size),
        Some(Overlay::Detail) => render_detail_overlay(f, app, size),
        Some(Overlay::Diff) => render_diff_overlay(f, app, size),
        None => {}
    }
}

fn render_ticker(f: &mut Frame, app: &App, area: Rect) {
    let text = app.ticker_text();
    let doubled = format!("{text}{text}");
    let offset = app.ticker_offset % text.len().max(1);
    let display: String = doubled.chars().skip(offset).take(area.width as usize).collect();

    let para = Paragraph::new(display)
        .style(Style::default().bg(Color::DarkGray).fg(Color::White));
    f.render_widget(para, area);
}

fn render_statusbar(f: &mut Frame, app: &App, area: Rect) {
    let (total, dirty, behind, _) = app.counts();
    let filter_str = if app.filter.is_empty() {
        String::new()
    } else {
        format!("  filter:\"{}\"", app.filter)
    };
    let fetch_str = if app.fetch { " [fetch]" } else { "" };
    let text = format!(
        " {} repos | dirty:{} behind:{} | sort:{}{}{} ",
        total, dirty, behind,
        app.sort_mode.label(), fetch_str, filter_str
    );

    let para = Paragraph::new(text)
        .style(Style::default().bg(Color::Blue).fg(Color::White).add_modifier(Modifier::BOLD));
    f.render_widget(para, area);
}

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

fn render_list(f: &mut Frame, app: &mut App, area: Rect) {
    let visible = area.height as usize;
    app.clamp_scroll(visible);

    let mut items: Vec<ListItem> = Vec::new();

    if app.show_header {
        let header = Line::from(vec![
            Span::styled(
                format!("{:<40} {:<12} {:<8} {:>5} {:>6} {:<8} {:<10}",
                    "REPO", "BRANCH", "SHA", "AHEAD", "BEHIND", "DIRTY", "STATUS"),
                Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            )
        ]);
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
        // +1 if header shown
        let display_idx = app.selected - app.scroll_offset + if app.show_header { 1 } else { 0 };
        list_state.select(Some(display_idx));
    }

    let list = List::new(items)
        .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
        .highlight_symbol("> ");

    f.render_stateful_widget(list, area, &mut list_state);
}

fn render_helpbar(f: &mut Frame, app: &App, area: Rect) {
    let text = if app.filter_mode {
        format!(" Filter: {}█", app.filter)
    } else {
        " j/k:move  Enter:detail  d:diff  s:sort  f:fetch  r:rescan  /:filter  ?:help  q:quit".to_string()
    };
    let para = Paragraph::new(text)
        .style(Style::default().bg(Color::DarkGray).fg(Color::White));
    f.render_widget(para, area);
}

fn render_help_overlay(f: &mut Frame, area: Rect) {
    let popup = centered_rect(60, 80, area);
    f.render_widget(Clear, popup);

    let text = vec![
        Line::from(Span::styled("Key Bindings", Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED))),
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
        Line::from(""),
        Line::from(Span::styled("  Press any key to close", Style::default().fg(Color::DarkGray))),
    ];

    let block = Block::default()
        .title(" Help ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let para = Paragraph::new(text)
        .block(block)
        .wrap(Wrap { trim: false });
    f.render_widget(para, popup);
}

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

fn render_diff_overlay(f: &mut Frame, app: &App, area: Rect) {
    let popup = centered_rect(80, 80, area);
    f.render_widget(Clear, popup);

    let inner_h = popup.height.saturating_sub(2) as usize;
    let lines: Vec<Line> = app.diff_lines
        .iter()
        .skip(app.diff_scroll)
        .take(inner_h)
        .map(|l| {
            let color = if l.starts_with('+') {
                Color::Green
            } else if l.starts_with('-') {
                Color::Red
            } else if l.starts_with('@') {
                Color::Cyan
            } else {
                Color::Reset
            };
            Line::from(Span::styled(l.as_str(), Style::default().fg(color)))
        })
        .collect();

    let title = if let Some(r) = app.selected_repo() {
        format!(" diff: {} ", r.repo)
    } else {
        " diff ".to_string()
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));

    let para = Paragraph::new(lines).block(block);
    f.render_widget(para, popup);
}

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
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max.saturating_sub(1)])
    }
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
