mod keygen;
mod search;
mod types;

use clap::Parser;

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

    /// Progress update interval in seconds
    #[arg(long = "progress-interval", default_value = "1.0")]
    progress_interval: f64,
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

fn main() {
    let cli = Cli::parse();

    let prefix = match validate_prefix(&cli.prefix) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    let num_threads = cli.threads.unwrap_or_else(|| {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
    });

    eprintln!(
        "Searching for prefix '{}' using {} thread{}...",
        prefix,
        num_threads,
        if num_threads == 1 { "" } else { "s" }
    );

    let result = search::search(&prefix, num_threads, move |stats| {
        eprintln!(
            "[{:.1}s] {} keys checked ({:.0} keys/sec)",
            stats.elapsed_secs, stats.attempts, stats.keys_per_sec,
        );
    }, cli.progress_interval);

    eprintln!(
        "\nFound match after {} attempts ({:.2}s)",
        result.attempts, result.elapsed_secs
    );

    if cli.json {
        println!("{}", serde_json::to_string_pretty(&result).unwrap());
    } else {
        println!("Public Key:  {}", result.public_key);
        println!("Private Key: {}", result.private_key);
    }
}
