mod types;
mod git;
mod scanner;
mod app;
mod ui;
mod output;

use std::io;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use clap::{Parser, ValueEnum};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::app::App;
use crate::scanner::{find_repos, is_aosp};
use crate::types::{OutputFormat, Overlay};

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Format {
    Tui,
    Tsv,
    Json,
    Table,
}

#[derive(Parser, Debug)]
#[command(name = "repo-report-tui", version = "0.3.0", about = "Git repository status reporter (Ratatui edition)")]
struct Cli {
    /// Root directory to scan
    #[arg(default_value = ".")]
    root: PathBuf,

    /// Output format (default: tui when stdout is a tty, tsv otherwise)
    #[arg(short = 'F', long)]
    format: Option<Format>,

    /// Fetch from remotes before reporting
    #[arg(short, long)]
    fetch: bool,

    /// Maximum depth to scan for repos
    #[arg(long, default_value = "5")]
    max_depth: usize,

    /// Maximum message column width (table format)
    #[arg(long, default_value = "60")]
    msg_width: usize,

    /// Non-interactive mode (equivalent to --format tsv)
    #[arg(long)]
    non_interactive: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let root = cli.root.canonicalize().unwrap_or(cli.root.clone());

    // Determine output format
    let is_tty = atty_check();
    let format = match cli.format {
        Some(Format::Tsv) => OutputFormat::Tsv,
        Some(Format::Json) => OutputFormat::Json,
        Some(Format::Table) => OutputFormat::Table,
        Some(Format::Tui) => OutputFormat::Tui,
        None if cli.non_interactive => OutputFormat::Tsv,
        None if is_tty => OutputFormat::Tui,
        None => OutputFormat::Tsv,
    };

    match format {
        OutputFormat::Tsv | OutputFormat::Json | OutputFormat::Table => {
            run_non_interactive(&root, cli.fetch, cli.max_depth, cli.msg_width, format)
        }
        OutputFormat::Tui => run_tui(&root, cli.fetch, cli.max_depth),
    }
}

fn run_non_interactive(
    root: &PathBuf,
    fetch: bool,
    max_depth: usize,
    msg_width: usize,
    format: OutputFormat,
) -> Result<()> {
    use rayon::prelude::*;
    use crate::git::scan_repo;

    let repos_paths = find_repos(root, max_depth);
    let mut repos: Vec<_> = repos_paths
        .par_iter()
        .map(|p| scan_repo(p, fetch, root))
        .collect();

    repos.sort_by(|a, b| a.repo.cmp(&b.repo));

    match format {
        OutputFormat::Tsv => output::print_tsv(&repos),
        OutputFormat::Json => output::print_json(&repos),
        OutputFormat::Table => output::print_table(&repos, msg_width),
        _ => unreachable!(),
    }

    // Exit code: 0 if all clean+up-to-date, 1 otherwise
    let all_good = repos.iter().all(|r| {
        !r.dirty
            && matches!(
                r.status,
                crate::types::RepoStatus::UpToDate | crate::types::RepoStatus::NoUpstream
            )
    });
    std::process::exit(if all_good { 0 } else { 1 });
}

fn run_tui(root: &PathBuf, fetch: bool, max_depth: usize) -> Result<()> {
    let aosp = is_aosp(root);

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Build app and start scan
    let mut app = App::new(root.clone(), fetch, max_depth, aosp);
    app.start_scan();

    // Sort mode cycling needs access to app, handle via event
    let result = run_event_loop(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<()> {
    loop {
        // Drain incoming scan results
        app.drain_scan();

        // Animate ticker
        app.tick();

        // Render
        terminal.draw(|f| ui::render(f, app))?;

        // Handle input (non-blocking, 100ms timeout)
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                handle_key(app, key.code, key.modifiers);
            }

            if let Event::Resize(_, _) = event::read().unwrap_or(Event::FocusLost) {
                // ratatui handles resize automatically via terminal.draw
            }
        }

        if app.should_quit {
            break;
        }
    }
    Ok(())
}

fn handle_key(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
    // Filter mode captures text
    if app.filter_mode {
        match code {
            KeyCode::Esc => {
                app.filter_mode = false;
                app.filter.clear();
                app.rebuild_filtered();
            }
            KeyCode::Enter => {
                app.filter_mode = false;
            }
            KeyCode::Backspace => {
                app.filter.pop();
                app.rebuild_filtered();
            }
            KeyCode::Char(c) => {
                app.filter.push(c);
                app.rebuild_filtered();
            }
            _ => {}
        }
        return;
    }

    // Overlay mode: only Esc / j / k handled
    if app.overlay.is_some() {
        match code {
            KeyCode::Esc | KeyCode::Char('q') => app.overlay = None,
            KeyCode::Char('j') | KeyCode::Down => {
                if app.overlay == Some(Overlay::Diff) {
                    let max = app.diff_lines.len().saturating_sub(1);
                    app.diff_scroll = (app.diff_scroll + 1).min(max);
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if app.overlay == Some(Overlay::Diff) {
                    app.diff_scroll = app.diff_scroll.saturating_sub(1);
                }
            }
            _ => {}
        }
        return;
    }

    // Normal mode
    match code {
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => app.should_quit = true,

        KeyCode::Char('j') | KeyCode::Down => app.move_down(),
        KeyCode::Char('k') | KeyCode::Up => app.move_up(),
        KeyCode::Char('g') => app.move_top(),
        KeyCode::Char('G') => app.move_bottom(),
        KeyCode::PageDown => app.page_down(10),
        KeyCode::PageUp => app.page_up(10),

        KeyCode::Enter => app.overlay = Some(Overlay::Detail),

        KeyCode::Char('d') => {
            app.load_diff();
            app.overlay = Some(Overlay::Diff);
        }

        KeyCode::Char('?') => app.overlay = Some(Overlay::Help),

        KeyCode::Char('s') => {
            app.sort_mode = app.sort_mode.next();
            app.sort();
        }

        KeyCode::Char('f') => {
            app.fetch = !app.fetch;
            app.start_scan();
        }

        KeyCode::Char('r') => {
            app.start_scan();
        }

        KeyCode::Char('/') => {
            app.filter_mode = true;
        }

        KeyCode::Esc => {
            if !app.filter.is_empty() {
                app.filter.clear();
                app.rebuild_filtered();
            }
        }

        KeyCode::Char('c') => {
            app.show_header = !app.show_header;
        }

        _ => {}
    }
}

fn atty_check() -> bool {
    use std::os::unix::io::AsRawFd;
    extern "C" { fn isatty(fd: i32) -> i32; }
    unsafe { isatty(io::stdout().as_raw_fd()) != 0 }
}
