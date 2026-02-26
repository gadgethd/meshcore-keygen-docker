mod keygen;
mod search;
mod types;

use std::io::{self, stdout};
use std::time::Duration;

use clap::Parser;
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Gauge, Paragraph},
};

use search::SearchHandle;

#[derive(Parser)]
#[command(name = "mc-keygen", version, about = "MeshCore vanity Ed25519 key generator")]
struct Cli {
    /// Hex prefix to search for (1-8 chars, 0-9/A-F)
    prefix: String,

    /// Number of worker threads (default: all cores)
    #[arg(short = 't', long = "threads")]
    threads: Option<usize>,

    /// Output result as JSON
    #[arg(long)]
    json: bool,
}

fn validate_prefix(prefix: &str) -> Result<String, String> {
    let upper = prefix.to_ascii_uppercase();

    if upper.is_empty() || upper.len() > 8 {
        return Err(format!(
            "prefix must be 1-8 hex characters, got {} characters",
            upper.len()
        ));
    }

    if !upper.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("prefix must be valid hex (0-9, A-F), got '{}'", prefix));
    }

    // Reject prefixes that would always start with 00 or FF
    if upper.starts_with("00") || upper.starts_with("FF") {
        return Err(format!(
            "prefix '{}' starts with 00 or FF, which are skipped by MeshCore",
            upper
        ));
    }

    Ok(upper)
}

fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

fn format_duration(secs: f64) -> String {
    if secs < 60.0 {
        format!("{:.1}s", secs)
    } else if secs < 3600.0 {
        let mins = (secs / 60.0).floor();
        let rem = secs - mins * 60.0;
        format!("{}m {:.0}s", mins as u64, rem)
    } else {
        let hours = (secs / 3600.0).floor();
        let rem = secs - hours * 3600.0;
        let mins = (rem / 60.0).floor();
        format!("{}h {}m", hours as u64, mins as u64)
    }
}

fn run_tui_search(
    prefix: &str,
    num_threads: usize,
    expected: u64,
) -> io::Result<types::SearchResult> {
    let handle = SearchHandle::start(prefix, num_threads);

    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    let result = loop {
        let stats = handle.stats(expected);
        let done = handle.is_done();

        terminal.draw(|frame| {
            let area = frame.area();

            let outer = Block::default()
                .title(" mc-keygen ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan));

            let inner = outer.inner(area);
            frame.render_widget(outer, area);

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([
                    Constraint::Length(1), // prefix line
                    Constraint::Length(1), // blank
                    Constraint::Length(1), // gauge
                    Constraint::Length(1), // blank
                    Constraint::Length(1), // keys checked
                    Constraint::Length(1), // speed
                    Constraint::Length(1), // elapsed
                    Constraint::Length(1), // est remaining
                    Constraint::Min(0),   // spacer
                ])
                .split(inner);

            // Prefix line
            let prefix_line = Line::from(vec![
                Span::styled("Searching for prefix: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    prefix,
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  ({} threads)", num_threads),
                    Style::default().fg(Color::DarkGray),
                ),
            ]);
            frame.render_widget(Paragraph::new(prefix_line), chunks[0]);

            // Progress gauge
            let exp = stats.expected_attempts;
            let ratio = if exp > 0 {
                (stats.attempts as f64 / exp as f64).min(1.0)
            } else {
                0.0
            };
            let pct_actual = if exp > 0 {
                stats.attempts as f64 / exp as f64 * 100.0
            } else {
                0.0
            };
            let gauge_label = format!(
                "{:.0}%  ({}/{})",
                pct_actual,
                format_number(stats.attempts),
                format_number(exp),
            );
            let gauge = Gauge::default()
                .gauge_style(Style::default().fg(Color::Green).bg(Color::DarkGray))
                .ratio(ratio)
                .label(gauge_label);
            frame.render_widget(gauge, chunks[2]);

            // Stats
            let keys_line = Line::from(vec![
                Span::styled("Keys checked:   ", Style::default().fg(Color::Gray)),
                Span::styled(
                    format_number(stats.attempts),
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                ),
            ]);
            frame.render_widget(Paragraph::new(keys_line), chunks[4]);

            let speed_line = Line::from(vec![
                Span::styled("Speed:          ", Style::default().fg(Color::Gray)),
                Span::styled(
                    format!("{} keys/sec", format_number(stats.keys_per_sec as u64)),
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                ),
            ]);
            frame.render_widget(Paragraph::new(speed_line), chunks[5]);

            let elapsed_line = Line::from(vec![
                Span::styled("Elapsed:        ", Style::default().fg(Color::Gray)),
                Span::styled(
                    format_duration(stats.elapsed_secs),
                    Style::default().fg(Color::White),
                ),
            ]);
            frame.render_widget(Paragraph::new(elapsed_line), chunks[6]);

            let remaining = if stats.keys_per_sec > 0.0 && stats.attempts < exp {
                let rem = (exp - stats.attempts) as f64 / stats.keys_per_sec;
                format_duration(rem)
            } else if stats.attempts >= exp {
                "any moment...".to_string()
            } else {
                "calculating...".to_string()
            };
            let remaining_line = Line::from(vec![
                Span::styled("Est. remaining: ", Style::default().fg(Color::Gray)),
                Span::styled(remaining, Style::default().fg(Color::White)),
            ]);
            frame.render_widget(Paragraph::new(remaining_line), chunks[7]);
        })?;

        if done {
            break handle.finish();
        }

        // Poll for Ctrl+C / 'q' to allow clean exit, otherwise tick every 50ms
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.code == KeyCode::Char('q')
                    || key.code == KeyCode::Char('c')
                        && key.modifiers.contains(event::KeyModifiers::CONTROL)
                {
                    // Restore terminal before exiting
                    disable_raw_mode()?;
                    execute!(stdout(), LeaveAlternateScreen)?;
                    std::process::exit(130);
                }
            }
        }
    };

    disable_raw_mode()?;
    execute!(stdout(), LeaveAlternateScreen)?;

    Ok(result)
}

fn print_colored_result(result: &types::SearchResult, prefix: &str) {
    use crossterm::style::{self, Stylize};

    // Green checkmark + bold "Match found!" line
    let attempts_str = format_number(result.attempts);
    let speed = if result.elapsed_secs > 0.0 {
        format_number((result.attempts as f64 / result.elapsed_secs) as u64)
    } else {
        "N/A".to_string()
    };

    eprintln!(
        "{}",
        style::style(format!(
            " ✓ Match found!  {} attempts in {} ({} keys/sec)",
            attempts_str,
            format_duration(result.elapsed_secs),
            speed,
        ))
        .green()
        .bold()
    );
    eprintln!();

    // Public key with prefix highlighted
    let prefix_len = prefix.len();
    let pk_prefix = &result.public_key[..prefix_len];
    let pk_rest = &result.public_key[prefix_len..];
    eprint!(
        "{}",
        style::style("Public Key:  ").dim()
    );
    eprint!(
        "{}",
        style::style(pk_prefix).green().bold()
    );
    eprintln!(
        "{}",
        style::style(pk_rest).white()
    );

    // Private key
    eprint!(
        "{}",
        style::style("Private Key: ").dim()
    );
    eprintln!(
        "{}",
        style::style(&result.private_key).white()
    );
}

fn print_colored_error(msg: &str) {
    use crossterm::style::{self, Stylize};
    eprintln!(
        "{}",
        style::style(format!(" ✗ Error: {}", msg)).red().bold()
    );
}

fn main() {
    let cli = Cli::parse();

    let prefix = match validate_prefix(&cli.prefix) {
        Ok(p) => p,
        Err(e) => {
            if cli.json {
                eprintln!("Error: {}", e);
            } else {
                print_colored_error(&e);
            }
            std::process::exit(1);
        }
    };

    let num_threads = cli.threads.unwrap_or_else(|| {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
    });

    let expected = 16u64.pow(prefix.len() as u32);

    if cli.json {
        // JSON mode: no TUI, no progress, just run and print result
        let handle = SearchHandle::start(&prefix, num_threads);
        let result = handle.finish();
        println!("{}", serde_json::to_string_pretty(&result).unwrap());
    } else {
        // TUI mode with progress bar
        let result = match run_tui_search(&prefix, num_threads, expected) {
            Ok(r) => r,
            Err(e) => {
                // If TUI fails (e.g., not a terminal), fall back to simple output
                eprintln!("TUI error: {}, falling back to simple mode", e);
                let handle = SearchHandle::start(&prefix, num_threads);
                handle.finish()
            }
        };
        print_colored_result(&result, &prefix);
    }
}
