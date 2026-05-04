mod checkpoint;
mod cpu;
mod deterministic;
#[cfg(feature = "cuda")]
mod gpu;
mod keygen;
#[cfg(feature = "metal")]
mod metal_gpu;
mod search;
mod types;
#[cfg(feature = "server")]
mod server {
    pub use mc_keygen::server::*;
}

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
#[command(
    name = "mc-keygen",
    version,
    about = "MeshCore vanity Ed25519 key generator"
)]
struct Cli {
    /// Hex prefix(es) to search for (1-64 chars, 0-9/A-F each)
    #[arg(required = true)]
    prefix: Vec<String>,

    /// Number of worker threads (default: all cores)
    #[arg(short = 't', long = "threads")]
    threads: Option<usize>,

    /// Output result as JSON
    #[arg(long)]
    json: bool,

    /// Force CPU-only search (no GPU even if available)
    #[cfg(any(feature = "cuda", feature = "metal"))]
    #[arg(long, conflicts_with = "gpu_only")]
    cpu_only: bool,

    /// Force GPU-only search (no CPU threads)
    #[cfg(any(feature = "cuda", feature = "metal"))]
    #[arg(long, conflicts_with = "cpu_only")]
    gpu_only: bool,

    /// Verify GPU keygen matches CPU (run 64 test seeds and compare)
    #[cfg(any(feature = "cuda", feature = "metal"))]
    #[arg(long)]
    verify: bool,

    /// Use deterministic seed+counter mode instead of random seeds
    #[arg(long)]
    deterministic: bool,

    /// Master seed for deterministic mode (64 hex chars). Random if not provided.
    #[arg(long, requires = "deterministic")]
    master_seed: Option<String>,

    /// Path to checkpoint file for save/resume
    #[arg(long, requires = "deterministic")]
    checkpoint: Option<String>,

    /// Seconds between checkpoint saves (default: 10)
    #[arg(long, default_value = "10", requires = "deterministic")]
    checkpoint_interval: u64,

    /// Resume from a checkpoint file
    #[arg(long, requires = "deterministic")]
    resume: Option<String>,

    /// Emit JSON progress lines to stdout
    #[arg(long)]
    json_progress: bool,

    /// Starting counter for deterministic mode
    #[arg(long, requires = "deterministic")]
    start_counter: Option<u64>,

    /// Worker ID for multi-worker setups (default: 0)
    #[arg(long, requires = "deterministic")]
    worker_id: Option<u64>,

    /// Total workers for chunk allocation (default: 1)
    #[arg(long, requires = "deterministic")]
    workers: Option<u64>,

    /// Maximum attempts before stopping
    #[arg(long)]
    max_attempts: Option<u64>,

    /// Maximum runtime in seconds
    #[arg(long)]
    max_runtime: Option<u64>,

    /// Start web server (requires 'server' feature)
    #[cfg(feature = "server")]
    #[arg(long)]
    serve: bool,
}

fn validate_prefix(prefix: &str) -> Result<String, String> {
    let upper = prefix.to_ascii_uppercase();

    if upper.is_empty() || upper.len() > 64 {
        return Err(format!(
            "prefix must be 1-64 hex characters, got {} characters",
            upper.len()
        ));
    }

    if upper.len() > 8 {
        let expected_f64 = 16_f64.powf(upper.len() as f64);
        eprintln!(
            "Warning: prefix length {} is computationally expensive. \
             Expected ~{:.2e} attempts on average.",
            upper.len(),
            expected_f64,
        );
    }

    if !upper.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!(
            "prefix must be valid hex (0-9, A-F), got '{}'",
            prefix
        ));
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

fn run_tui_loop(
    handle: SearchHandle,
    prefixes: &[String],
    expected: u64,
    mode_label: &str,
) -> io::Result<Result<types::SearchResult, types::SearchError>> {
    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    let prefix_display = if prefixes.len() == 1 {
        prefixes[0].clone()
    } else {
        prefixes.join(", ")
    };
    let prefix_label = if prefixes.len() == 1 {
        "Searching for prefix: ".to_string()
    } else {
        format!("Searching for {} prefixes: ", prefixes.len())
    };

    let result = loop {
        let stats = handle.stats(expected);
        let done = handle.is_done();

        let mode_label_owned = mode_label.to_string();
        let prefix_label_owned = prefix_label.clone();
        let prefix_display_owned = prefix_display.clone();
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
                    Constraint::Min(0),    // spacer
                ])
                .split(inner);

            // Prefix line
            let prefix_line = Line::from(vec![
                Span::styled(&*prefix_label_owned, Style::default().fg(Color::Gray)),
                Span::styled(
                    &*prefix_display_owned,
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  ({})", mode_label_owned),
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
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
            ]);
            frame.render_widget(Paragraph::new(keys_line), chunks[4]);

            let speed_line = Line::from(vec![
                Span::styled("Speed:          ", Style::default().fg(Color::Gray)),
                Span::styled(
                    format!("{} keys/sec", format_number(stats.keys_per_sec as u64)),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
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

fn print_colored_result(result: &types::SearchResult) {
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

    // Public key with matched prefix highlighted
    let prefix_len = result.matched_prefix.len();
    let pk_prefix = &result.public_key[..prefix_len];
    let pk_rest = &result.public_key[prefix_len..];
    eprint!("{}", style::style("Public Key:  ").dim());
    eprint!("{}", style::style(pk_prefix).green().bold());
    eprintln!("{}", style::style(pk_rest).white());

    // Private key
    eprint!("{}", style::style("Private Key: ").dim());
    eprintln!("{}", style::style(&result.private_key).white());

    if let Some(seed) = &result.seed {
        eprint!("{}", style::style("Seed:        ").dim());
        eprintln!("{}", style::style(seed).white());
    }
}

fn print_colored_error(msg: &str) {
    use crossterm::style::{self, Stylize};
    eprintln!(
        "{}",
        style::style(format!(" ✗ Error: {}", msg)).red().bold()
    );
}

#[allow(unused_variables)]
fn try_init_gpu(prefixes: &[String]) -> Vec<Box<dyn search::GpuSearcher>> {
    #[cfg(feature = "metal")]
    {
        match metal_gpu::MetalSearcher::new(prefixes) {
            Ok(s) => return vec![Box::new(s)],
            Err(e) => {
                eprintln!("Warning: Metal GPU unavailable ({}), using CPU only", e);
                return vec![];
            }
        }
    }
    #[cfg(feature = "cuda")]
    {
        match gpu::CudaSearcher::new(prefixes) {
            Ok(s) => return vec![Box::new(s)],
            Err(e) => {
                eprintln!("Warning: CUDA GPU unavailable ({}), using CPU only", e);
                return vec![];
            }
        }
    }
    #[cfg(not(any(feature = "cuda", feature = "metal")))]
    {
        vec![]
    }
}

fn gpu_names_label(searchers: &[Box<dyn search::GpuSearcher>]) -> String {
    searchers
        .iter()
        .map(|g| g.device_name())
        .collect::<Vec<_>>()
        .join(", ")
}

fn main() {
    let cli = Cli::parse();

    // --serve mode: start the web server
    #[cfg(feature = "server")]
    if cli.serve {
        let db_path = std::env::var("DATABASE_PATH").unwrap_or_else(|_| "/data/app.db".to_string());
        let bind = std::env::var("APP_BIND").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
        let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
        rt.block_on(async {
            if let Err(e) = server::run(&bind, &db_path).await {
                eprintln!("Server error: {}", e);
                std::process::exit(1);
            }
        });
        return;
    }

    let mut prefixes = Vec::new();
    for raw in &cli.prefix {
        match validate_prefix(raw) {
            Ok(p) => prefixes.push(p),
            Err(e) => {
                if cli.json || cli.json_progress {
                    eprintln!("Error: {}", e);
                } else {
                    print_colored_error(&e);
                }
                std::process::exit(1);
            }
        }
    }

    // CPU config with reservation
    let cpu_config = cpu::CpuConfig::detect();
    let num_threads = cli
        .threads
        .unwrap_or_else(|| cpu_config.available_workers().max(1));

    // Expected attempts: use shortest prefix length, divided by count of same-length prefixes
    let min_len = prefixes.iter().map(|p| p.len()).min().unwrap();
    let same_len_count = prefixes.iter().filter(|p| p.len() == min_len).count() as u64;
    let expected = 16u64.pow(min_len as u32) / same_len_count;

    let prefix_count_label = if prefixes.len() == 1 {
        String::new()
    } else {
        format!(", {} prefixes", prefixes.len())
    };

    #[cfg(any(feature = "cuda", feature = "metal"))]
    let cpu_only = cli.cpu_only;
    #[cfg(not(any(feature = "cuda", feature = "metal")))]
    let cpu_only = true;

    #[cfg(any(feature = "cuda", feature = "metal"))]
    let gpu_only = cli.gpu_only;
    #[cfg(not(any(feature = "cuda", feature = "metal")))]
    let gpu_only = false;

    #[cfg(any(feature = "cuda", feature = "metal"))]
    if cli.verify {
        eprint!("Compiling GPU kernel and running verification... ");
        #[cfg(feature = "cuda")]
        let result = gpu::verify_gpu_keygen().map_err(|e| format!("{}", e));
        #[cfg(all(feature = "metal", not(feature = "cuda")))]
        let result = metal_gpu::verify_gpu_keygen().map_err(|e| format!("{}", e));
        match result {
            Ok(()) => {
                eprintln!("PASSED");
                std::process::exit(0);
            }
            Err(e) => {
                eprintln!("FAILED: {}", e);
                std::process::exit(1);
            }
        }
    }

    // --- Deterministic mode ---
    if cli.deterministic {
        let mut state = if let Some(ref checkpoint_path) = cli.resume {
            let path = std::path::PathBuf::from(checkpoint_path);
            match checkpoint::Checkpoint::load(&path) {
                Ok(ckpt) => {
                    eprintln!("Loaded checkpoint from {}", checkpoint_path);
                    ckpt.to_deterministic_state().unwrap_or_else(|e| {
                        eprintln!("Warning: failed to restore state: {}", e);
                        deterministic::DeterministicState::new()
                    })
                }
                Err(e) => {
                    eprintln!("Warning: failed to load checkpoint ({}), starting fresh", e);
                    deterministic::DeterministicState::new()
                }
            }
        } else if let Some(ref hex) = cli.master_seed {
            deterministic::DeterministicState::from_hex_seed(hex).unwrap_or_else(|e| {
                eprintln!("Error: invalid master seed: {}", e);
                std::process::exit(1);
            })
        } else {
            eprintln!("Generating random master seed...");
            deterministic::DeterministicState::new()
        };

        if let Some(counter) = cli.start_counter {
            state.set_counter(counter);
        }
        if let Some(wid) = cli.worker_id {
            state.worker_id = wid;
        }

        let checkpoint_path = cli.checkpoint.as_ref().map(std::path::PathBuf::from);
        let handle = SearchHandle::start_deterministic(
            &prefixes,
            state,
            num_threads,
            checkpoint_path,
            cli.checkpoint_interval,
            cli.json_progress,
        );

        let mode_label = format!(
            "deterministic, {} threads{}",
            num_threads, prefix_count_label
        );

        if cli.json_progress {
            // JSON progress mode: emit progress lines until done, then result
            while !handle.is_done() {
                if let Some(line) = handle.json_progress("cpu", "none") {
                    println!("{}", line);
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            if let Some(line) = handle.json_result() {
                println!("{}", line);
            }
            match handle.finish() {
                Ok(_) => std::process::exit(0),
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        } else if cli.json {
            match handle.finish() {
                Ok(result) => println!("{}", serde_json::to_string_pretty(&result).unwrap()),
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        } else {
            let search_result = match run_tui_loop(handle, &prefixes, expected, &mode_label) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("TUI error: {}", e);
                    std::process::exit(1);
                }
            };
            match search_result {
                Ok(result) => print_colored_result(&result),
                Err(e) => {
                    print_colored_error(&format!("{}", e));
                    std::process::exit(1);
                }
            }
        }
        return;
    }

    // --- Non-deterministic mode (original behavior) ---
    let gpu_searchers = if cpu_only {
        vec![]
    } else {
        try_init_gpu(&prefixes)
    };

    let (handle, mode_label) = if gpu_only {
        if gpu_searchers.is_empty() {
            let msg = "--gpu-only requested but no GPU available";
            if cli.json {
                eprintln!("Error: {}", msg);
            } else {
                print_colored_error(msg);
            }
            std::process::exit(1);
        }
        let label = format!("{}{}", gpu_names_label(&gpu_searchers), prefix_count_label);
        (SearchHandle::start_gpu(&prefixes, gpu_searchers), label)
    } else if gpu_searchers.is_empty() {
        let label = format!("{} threads{}", num_threads, prefix_count_label);
        (SearchHandle::start(&prefixes, num_threads), label)
    } else {
        let gpu_label = gpu_names_label(&gpu_searchers);
        let label = format!(
            "{} + {} threads{}",
            gpu_label, num_threads, prefix_count_label
        );
        (
            SearchHandle::start_hybrid(&prefixes, num_threads, gpu_searchers),
            label,
        )
    };

    if cli.json {
        match handle.finish() {
            Ok(result) => println!("{}", serde_json::to_string_pretty(&result).unwrap()),
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        let search_result = match run_tui_loop(handle, &prefixes, expected, &mode_label) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("TUI error: {}", e);
                std::process::exit(1);
            }
        };
        match search_result {
            Ok(result) => print_colored_result(&result),
            Err(e) => {
                print_colored_error(&format!("{}", e));
                std::process::exit(1);
            }
        }
    }
}

#[cfg(test)]
mod validation_tests {
    use super::*;

    #[test]
    fn valid_short_prefix() {
        assert!(validate_prefix("AB").is_ok());
        assert!(validate_prefix("C0DE").is_ok());
        assert!(validate_prefix("1A2B3C4D").is_ok());
    }

    #[test]
    fn valid_long_prefix() {
        // 9 chars (e.g. C0DEBA5ED)
        assert!(validate_prefix("C0DEBA5ED").is_ok());
        // 16 chars
        assert!(validate_prefix("C0DEBA5EDC0DEBA5E").is_ok());
        // 64 chars
        assert!(validate_prefix(&"A".repeat(64)).is_ok());
    }

    #[test]
    fn rejects_too_long_prefix() {
        assert!(validate_prefix(&"A".repeat(65)).is_err());
    }

    #[test]
    fn rejects_empty_prefix() {
        assert!(validate_prefix("").is_err());
    }

    #[test]
    fn rejects_non_hex_prefix() {
        assert!(validate_prefix("GH").is_err());
        assert!(validate_prefix("ZZZZ").is_err());
        assert!(validate_prefix("0xBE").is_err());
    }

    #[test]
    fn rejects_00_prefix() {
        assert!(validate_prefix("00").is_err());
        assert!(validate_prefix("00AB").is_err());
    }

    #[test]
    fn rejects_ff_prefix() {
        assert!(validate_prefix("FF").is_err());
        assert!(validate_prefix("FFAB").is_err());
    }

    #[test]
    fn case_insensitive_accept() {
        assert!(validate_prefix("ab").is_ok());
        assert!(validate_prefix("deadbeef").is_ok());
        assert!(validate_prefix("C0deBa5ed").is_ok());
    }

    #[test]
    fn single_char_prefix_ok() {
        assert!(validate_prefix("A").is_ok());
        assert!(validate_prefix("0").is_ok());
        assert!(validate_prefix("F").is_ok());
    }
}
