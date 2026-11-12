use std::ops::Range;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};

use crossbeam_channel::Sender;
use ignore::{DirEntry, ParallelVisitor, ParallelVisitorBuilder, WalkBuilder, WalkState};

use crate::matcher::Matcher;

/// Restricts results to files or directories.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum FileType {
    #[value(name = "f", alias = "file")]
    File,
    #[value(name = "d", alias = "dir")]
    Dir,
}

pub struct Options {
    pub roots: Vec<PathBuf>,
    pub hidden: bool,
    pub gitignore: bool,
    pub max_depth: Option<usize>,
    pub threads: usize,
    pub file_type: Option<FileType>,
    /// Lowercase-insensitive extensions, without the leading dot.
    pub extensions: Vec<String>,
    pub max_results: Option<u64>,
}

/// A matching entry, sent from the worker threads to the printer.
pub struct Hit {
    pub path: PathBuf,
    /// Byte length of the file name (as lossy UTF-8) at the end of `path`.
    pub name_len: usize,
    /// Byte range of the match inside the file name.
    pub name_match: Range<usize>,
}

#[derive(Default)]
pub struct Stats {
    pub scanned: AtomicU64,
    pub found: AtomicU64,
    pub skipped: AtomicU64,
    /// Set when the search should stop early (max results reached, or
    /// stdout closed).
    pub stop: AtomicBool,
}

/// Walks all roots in parallel and sends every match to `tx`.
/// Blocks until the walk finishes.
pub fn run(opts: &Options, matcher: &Matcher, stats: &Stats, tx: Sender<Hit>) {
    let mut builder = WalkBuilder::new(&opts.roots[0]);
    for root in &opts.roots[1..] {
        builder.add(root);
    }
    builder
        .hidden(!opts.hidden)
        .ignore(false)
        .git_ignore(opts.gitignore)
        .git_global(opts.gitignore)
        .git_exclude(opts.gitignore)
        .parents(opts.gitignore)
        .require_git(false)
        .follow_links(false)
        .max_depth(opts.max_depth)
        .threads(opts.threads);

    let mut workers = WorkerBuilder {
        opts,
        matcher,
        stats,
        tx,
    };
    builder.build_parallel().visit(&mut workers);
}

struct WorkerBuilder<'a> {
    opts: &'a Options,
    matcher: &'a Matcher,
    stats: &'a Stats,
    tx: Sender<Hit>,
}

impl<'a> ParallelVisitorBuilder<'a> for WorkerBuilder<'a> {
    fn build(&mut self) -> Box<dyn ParallelVisitor + 'a> {
        Box::new(Worker {
            opts: self.opts,
            matcher: self.matcher,
            stats: self.stats,
            tx: self.tx.clone(),
            scanned: 0,
            skipped: 0,
        })
    }
}

/// One per thread. Counters are kept locally and merged in batches, so the
/// hot path almost never touches a shared cache line.
struct Worker<'a> {
    opts: &'a Options,
    matcher: &'a Matcher,
    stats: &'a Stats,
    tx: Sender<Hit>,
    scanned: u64,
    skipped: u64,
}

impl ParallelVisitor for Worker<'_> {
    fn visit(&mut self, entry: Result<DirEntry, ignore::Error>) -> WalkState {
        if self.stats.stop.load(Relaxed) {
            return WalkState::Quit;
        }
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => {
                // Usually "access denied" on system folders.
                self.skipped += 1;
                return WalkState::Continue;
            }
        };
        // The root itself is not a result.
        if entry.depth() == 0 {
            return WalkState::Continue;
        }
        self.scanned += 1;
        // Publish progress now and then (cheap), so a UI can show live counts.
        if self.scanned == FLUSH_EVERY {
            self.flush();
        }

        if let Some(wanted) = self.opts.file_type {
            let is_dir = entry.file_type().is_some_and(|t| t.is_dir());
            if is_dir != (wanted == FileType::Dir) {
                return WalkState::Continue;
            }
        }

        let name = entry.file_name().to_string_lossy();
        if !extension_matches(&name, &self.opts.extensions) {
            return WalkState::Continue;
        }
        let Some(name_match) = self.matcher.find(&name) else {
            return WalkState::Continue;
        };
        let name_len = name.len();
        drop(name);

        let found = self.stats.found.fetch_add(1, Relaxed) + 1;
        if let Some(max) = self.opts.max_results {
            if found > max {
                self.stats.stop.store(true, Relaxed);
                return WalkState::Quit;
            }
        }

        let hit = Hit {
            path: entry.into_path(),
            name_len,
            name_match,
        };
        if self.tx.send(hit).is_err() {
            // The printer is gone (e.g. stdout was closed).
            self.stats.stop.store(true, Relaxed);
            return WalkState::Quit;
        }
        if self.opts.max_results == Some(found) {
            self.stats.stop.store(true, Relaxed);
            return WalkState::Quit;
        }
        WalkState::Continue
    }
}

/// How many entries a worker counts locally before publishing them.
const FLUSH_EVERY: u64 = 4096;

impl Worker<'_> {
    fn flush(&mut self) {
        self.stats.scanned.fetch_add(self.scanned, Relaxed);
        self.stats.skipped.fetch_add(self.skipped, Relaxed);
        self.scanned = 0;
        self.skipped = 0;
    }
}

impl Drop for Worker<'_> {
    fn drop(&mut self) {
        self.flush();
    }
}

/// Turns user input like `["pdf", ".DOCX", "jpg, png"]` into clean
/// extensions without dots: `["pdf", "DOCX", "jpg", "png"]`.
pub fn parse_extensions<'a>(items: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    items
        .into_iter()
        .flat_map(|s| s.split([',', ';', ' ']))
        .map(|e| e.trim().trim_start_matches('.'))
        .filter(|e| !e.is_empty())
        .map(String::from)
        .collect()
}

/// `extensions` must already be stripped of leading dots.
fn extension_matches(name: &str, extensions: &[String]) -> bool {
    if extensions.is_empty() {
        return true;
    }
    match name.rsplit_once('.') {
        // ".gitignore" has no extension, like std::path::Path::extension.
        Some((stem, ext)) if !stem.is_empty() => {
            extensions.iter().any(|e| e.eq_ignore_ascii_case(ext))
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{extension_matches, parse_extensions};

    fn exts(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_extension_lists() {
        assert_eq!(
            parse_extensions(["pdf", ".DOCX", "jpg, png", " ;.txt "]),
            exts(&["pdf", "DOCX", "jpg", "png", "txt"])
        );
        assert!(parse_extensions([""]).is_empty());
    }

    #[test]
    fn no_filter_accepts_everything() {
        assert!(extension_matches("anything", &[]));
    }

    #[test]
    fn extension_is_case_insensitive() {
        assert!(extension_matches("Rapor.PDF", &exts(&["pdf"])));
        assert!(extension_matches("a.tar.gz", &exts(&["gz"])));
        assert!(!extension_matches("a.tar.gz", &exts(&["tar"])));
    }

    #[test]
    fn multiple_extensions() {
        let list = exts(&["pdf", "docx"]);
        assert!(extension_matches("a.docx", &list));
        assert!(!extension_matches("a.txt", &list));
    }

    #[test]
    fn dotfiles_and_extensionless_names() {
        let list = exts(&["gitignore"]);
        assert!(!extension_matches(".gitignore", &list));
        assert!(!extension_matches("Makefile", &exts(&["txt"])));
    }
}
