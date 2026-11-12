//! End-to-end tests: build a small directory tree and run the real binary.

use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use tempfile::TempDir;

/// Layout:
/// ```text
/// docs/report_2025.pdf
/// docs/Report.docx
/// docs/notes.txt
/// archive/old/report.txt      (ignored by .gitignore)
/// reports/                    (directory)
/// .hidden/report_secret.txt   (hidden)
/// .gitignore                  ("archive/")
/// ```
fn fixture() -> TempDir {
    let dir = tempfile::Builder::new().prefix("ff").tempdir().unwrap();
    let root = dir.path();
    for d in ["docs", "archive/old", "reports", ".hidden"] {
        fs::create_dir_all(root.join(d)).unwrap();
    }
    for f in [
        "docs/report_2025.pdf",
        "docs/Report.docx",
        "docs/notes.txt",
        "archive/old/report.txt",
        ".hidden/report_secret.txt",
    ] {
        fs::write(root.join(f), "").unwrap();
    }
    fs::write(root.join(".gitignore"), "archive/\n").unwrap();
    dir
}

/// Runs fastfind on `root` and returns (exit code, sorted output paths).
fn run(root: &Path, args: &[&str]) -> (i32, Vec<PathBuf>) {
    let output = Command::cargo_bin("fastfind")
        .unwrap()
        .args(["--color", "never", "-q"])
        .args(args)
        .arg(root)
        .output()
        .unwrap();
    let mut paths: Vec<PathBuf> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(PathBuf::from)
        .collect();
    paths.sort();
    (output.status.code().unwrap(), paths)
}

/// File names of the results, sorted.
fn names(paths: &[PathBuf]) -> Vec<String> {
    let mut v: Vec<String> = paths
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

#[test]
fn substring_search_is_case_insensitive() {
    let dir = fixture();
    let (code, paths) = run(dir.path(), &["report"]);
    assert_eq!(code, 0);
    assert_eq!(
        names(&paths),
        ["Report.docx", "report.txt", "report_2025.pdf", "reports"]
    );
}

#[test]
fn results_are_absolute_paths() {
    let dir = fixture();
    let (_, paths) = run(dir.path(), &["report"]);
    for p in &paths {
        assert!(p.is_absolute(), "{} is not absolute", p.display());
        assert!(p.exists(), "{} does not exist", p.display());
    }
}

#[test]
fn case_sensitive() {
    let dir = fixture();
    let (_, paths) = run(dir.path(), &["-s", "Report"]);
    assert_eq!(names(&paths), ["Report.docx"]);
}

#[test]
fn extension_filter() {
    let dir = fixture();
    let (_, paths) = run(dir.path(), &["-e", "pdf", "report"]);
    assert_eq!(names(&paths), ["report_2025.pdf"]);

    let (_, paths) = run(dir.path(), &["-e", ".PDF", "-e", "docx", "report"]);
    assert_eq!(names(&paths), ["Report.docx", "report_2025.pdf"]);
}

#[test]
fn type_filter() {
    let dir = fixture();
    let (_, paths) = run(dir.path(), &["-t", "d", "report"]);
    assert_eq!(names(&paths), ["reports"]);

    let (_, paths) = run(dir.path(), &["-t", "f", "report"]);
    assert_eq!(
        names(&paths),
        ["Report.docx", "report.txt", "report_2025.pdf"]
    );
}

#[test]
fn max_depth() {
    let dir = fixture();
    let (_, paths) = run(dir.path(), &["-d", "1", "report"]);
    assert_eq!(names(&paths), ["reports"]);
}

#[test]
fn hidden_entries_are_skipped_by_default() {
    let dir = fixture();
    let (_, paths) = run(dir.path(), &["secret"]);
    assert!(paths.is_empty());

    let (_, paths) = run(dir.path(), &["-H", "secret"]);
    assert_eq!(names(&paths), ["report_secret.txt"]);
}

#[test]
fn gitignore_is_opt_in() {
    let dir = fixture();
    let (_, paths) = run(dir.path(), &["--gitignore", "-t", "f", "report"]);
    assert_eq!(names(&paths), ["Report.docx", "report_2025.pdf"]);
}

#[test]
fn max_results() {
    let dir = fixture();
    let (code, paths) = run(dir.path(), &["-n", "2", "report"]);
    assert_eq!(code, 0);
    assert_eq!(paths.len(), 2);
}

#[test]
fn glob_mode() {
    let dir = fixture();
    let (_, paths) = run(dir.path(), &["-g", "*.pdf"]);
    assert_eq!(names(&paths), ["report_2025.pdf"]);

    let (_, paths) = run(dir.path(), &["-g", "report*"]);
    assert_eq!(
        names(&paths),
        ["Report.docx", "report.txt", "report_2025.pdf", "reports"]
    );
}

#[test]
fn regex_mode() {
    let dir = fixture();
    let (_, paths) = run(dir.path(), &["-r", r"^report_\d+"]);
    assert_eq!(names(&paths), ["report_2025.pdf"]);
}

#[test]
fn no_match_exits_with_1() {
    let dir = fixture();
    let (code, paths) = run(dir.path(), &["does-not-exist"]);
    assert_eq!(code, 1);
    assert!(paths.is_empty());
}

#[test]
fn errors_exit_with_2() {
    let dir = fixture();
    let (code, _) = run(&dir.path().join("missing"), &["x"]);
    assert_eq!(code, 2);

    let (code, _) = run(dir.path(), &["-r", "(unclosed"]);
    assert_eq!(code, 2);
}

#[test]
fn summary_goes_to_stderr() {
    let dir = fixture();
    let output = Command::cargo_bin("fastfind")
        .unwrap()
        .args(["--color", "never", "report"])
        .arg(dir.path())
        .output()
        .unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("4 results"), "stderr was: {stderr}");
    assert!(stderr.contains("entries scanned"));
}
