mod cli;
mod output;

use std::io::{self, BufWriter};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::atomic::Ordering::Relaxed;
use std::time::Instant;

use anstream::{AutoStream, ColorChoice};
use clap::Parser;

use cli::{Args, ColorWhen};
use fastfind::matcher::{Matcher, Mode};
use fastfind::search::{self, Options, Stats};
use output::{group_digits, Printer};

fn main() -> ExitCode {
    let args = Args::parse();
    match run(args) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(err) => {
            eprintln!("fastfind: error: {err}");
            ExitCode::from(2)
        }
    }
}

/// Returns `Ok(true)` if at least one match was printed.
fn run(args: Args) -> Result<bool, String> {
    let started = Instant::now();

    let mode = if args.glob {
        Mode::Glob
    } else if args.regex {
        Mode::Regex
    } else {
        Mode::Substring
    };
    let matcher = Matcher::new(&args.pattern, mode, args.case_sensitive)?;

    let opts = Options {
        roots: resolve_roots(args.paths)?,
        hidden: args.hidden,
        gitignore: args.gitignore,
        max_depth: args.max_depth,
        threads: args.threads,
        file_type: args.file_type,
        extensions: search::parse_extensions(args.extensions.iter().map(String::as_str)),
        max_results: args.max_results,
    };

    let choice = match args.color {
        ColorWhen::Auto => ColorChoice::Auto,
        ColorWhen::Always => ColorChoice::Always,
        ColorWhen::Never => ColorChoice::Never,
    };
    let stdout = AutoStream::new(io::stdout().lock(), choice);
    let color = stdout.current_choice() != ColorChoice::Never;
    let mut printer = Printer::new(BufWriter::with_capacity(64 * 1024, stdout), color);

    let stats = Stats::default();
    let (tx, rx) = crossbeam_channel::unbounded();
    let mut printed = 0u64;

    std::thread::scope(|scope| {
        scope.spawn(|| search::run(&opts, &matcher, &stats, tx));
        for hit in rx {
            if printer.print(&hit).is_err() {
                // stdout closed (e.g. piped into `head`): stop searching.
                stats.stop.store(true, Relaxed);
                break;
            }
            printed += 1;
        }
    });
    let _ = printer.flush();

    if !args.quiet {
        let mut summary = format!(
            "{} results · {} entries scanned",
            group_digits(printed),
            group_digits(stats.scanned.load(Relaxed))
        );
        let skipped = stats.skipped.load(Relaxed);
        if skipped > 0 {
            summary.push_str(&format!(" · {} skipped", group_digits(skipped)));
        }
        summary.push_str(&format!(" · {:.2}s", started.elapsed().as_secs_f64()));
        eprintln!("{summary}");
    }

    Ok(printed > 0)
}

/// Turns the given paths into absolute paths (current directory if none).
fn resolve_roots(paths: Vec<PathBuf>) -> Result<Vec<PathBuf>, String> {
    let paths = if paths.is_empty() {
        vec![PathBuf::from(".")]
    } else {
        paths
    };
    paths
        .into_iter()
        .map(|p| {
            if !p.is_dir() {
                return Err(format!("not a directory: {}", p.display()));
            }
            std::path::absolute(&p).map_err(|e| format!("{}: {e}", p.display()))
        })
        .collect()
}
