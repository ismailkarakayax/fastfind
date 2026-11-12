use std::path::PathBuf;

use clap::{Parser, ValueEnum};
use fastfind::search::FileType;

/// Blazing fast, parallel file name search.
///
/// Searches file and directory names (not contents) under the given paths
/// and prints the full path of every match.
#[derive(Parser, Debug)]
#[command(name = "fastfind", version, about, long_about = None)]
pub struct Args {
    /// Text to look for in file names (case-insensitive substring by default)
    pub pattern: String,

    /// Directories to search [default: current directory]
    pub paths: Vec<PathBuf>,

    /// Treat PATTERN as a glob, matched against the whole name (e.g. "*.pdf")
    #[arg(short = 'g', long, conflicts_with = "regex")]
    pub glob: bool,

    /// Treat PATTERN as a regular expression
    #[arg(short = 'r', long)]
    pub regex: bool,

    /// Match case-sensitively
    #[arg(short = 's', long)]
    pub case_sensitive: bool,

    /// Only show entries with this extension (repeatable: -e pdf -e docx)
    #[arg(short = 'e', long = "ext", value_name = "EXT")]
    pub extensions: Vec<String>,

    /// Only show files (f) or directories (d)
    #[arg(short = 't', long = "type", value_enum, value_name = "TYPE")]
    pub file_type: Option<FileType>,

    /// Maximum search depth (1 = only direct children)
    #[arg(short = 'd', long, value_name = "N")]
    pub max_depth: Option<usize>,

    /// Also search hidden files and directories
    #[arg(short = 'H', long)]
    pub hidden: bool,

    /// Respect .gitignore rules
    #[arg(long)]
    pub gitignore: bool,

    /// Stop after N results
    #[arg(short = 'n', long, value_name = "N", value_parser = clap::value_parser!(u64).range(1..))]
    pub max_results: Option<u64>,

    /// Number of worker threads [default: number of CPU cores]
    #[arg(
        short = 'j',
        long,
        value_name = "N",
        default_value_t = 0,
        hide_default_value = true
    )]
    pub threads: usize,

    /// When to use colors
    #[arg(long, value_enum, value_name = "WHEN", default_value_t = ColorWhen::Auto)]
    pub color: ColorWhen,

    /// Don't print the summary line
    #[arg(short = 'q', long)]
    pub quiet: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum ColorWhen {
    Auto,
    Always,
    Never,
}
