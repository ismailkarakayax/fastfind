use std::ops::Range;

use globset::{GlobBuilder, GlobMatcher};
use regex::{Regex, RegexBuilder};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Substring,
    Glob,
    Regex,
}

/// Matches a single file name (never the full path).
pub enum Matcher {
    Regex(Regex),
    Glob(GlobMatcher),
}

impl Matcher {
    pub fn new(pattern: &str, mode: Mode, case_sensitive: bool) -> Result<Self, String> {
        match mode {
            // A substring search is an escaped regex: the regex engine turns it
            // into a SIMD-accelerated literal search and gives us exact byte
            // ranges for highlighting, even with case-insensitive Unicode.
            Mode::Substring => build_regex(&regex::escape(pattern), case_sensitive),
            Mode::Regex => build_regex(pattern, case_sensitive),
            Mode::Glob => GlobBuilder::new(pattern)
                .case_insensitive(!case_sensitive)
                .literal_separator(true)
                .build()
                .map(|glob| Matcher::Glob(glob.compile_matcher()))
                .map_err(|e| format!("invalid glob pattern: {e}")),
        }
    }

    /// Returns the byte range of the match inside `name`, or `None`.
    /// Globs always match the whole name.
    #[inline]
    pub fn find(&self, name: &str) -> Option<Range<usize>> {
        match self {
            Matcher::Regex(re) => re.find(name).map(|m| m.range()),
            Matcher::Glob(glob) => glob.is_match(name).then_some(0..name.len()),
        }
    }
}

fn build_regex(pattern: &str, case_sensitive: bool) -> Result<Matcher, String> {
    RegexBuilder::new(pattern)
        .case_insensitive(!case_sensitive)
        .build()
        .map(Matcher::Regex)
        .map_err(|e| format!("invalid regex: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(pattern: &str, mode: Mode, cs: bool) -> Matcher {
        Matcher::new(pattern, mode, cs).unwrap()
    }

    #[test]
    fn substring_is_case_insensitive_by_default() {
        let matcher = m("report", Mode::Substring, false);
        assert_eq!(matcher.find("Report.docx"), Some(0..6));
        assert_eq!(matcher.find("old_REPORT.txt"), Some(4..10));
        assert_eq!(matcher.find("notes.txt"), None);
    }

    #[test]
    fn substring_case_sensitive() {
        let matcher = m("Report", Mode::Substring, true);
        assert_eq!(matcher.find("Report.docx"), Some(0..6));
        assert_eq!(matcher.find("report.docx"), None);
    }

    #[test]
    fn substring_escapes_regex_metacharacters() {
        let matcher = m("a+b", Mode::Substring, false);
        assert!(matcher.find("a+b.txt").is_some());
        assert!(matcher.find("aab.txt").is_none());
    }

    #[test]
    fn substring_handles_turkish_characters() {
        let matcher = m("şirket", Mode::Substring, false);
        // "Şirket" is 7 bytes: 'Ş' takes 2 bytes in UTF-8.
        assert_eq!(matcher.find("Şirket_raporu.txt"), Some(0..7));

        let matcher = m("ğ", Mode::Substring, false);
        assert!(matcher.find("DAĞ.jpg").is_some());

        let matcher = m("dosyalarım", Mode::Substring, false);
        assert!(matcher.find("eski_dosyalarım").is_some());
    }

    #[test]
    fn glob_matches_whole_name() {
        let matcher = m("*.pdf", Mode::Glob, false);
        assert_eq!(matcher.find("Rapor.PDF"), Some(0..9));
        assert_eq!(matcher.find("rapor.pdf.bak"), None);
    }

    #[test]
    fn glob_case_sensitive() {
        let matcher = m("*.pdf", Mode::Glob, true);
        assert!(matcher.find("a.pdf").is_some());
        assert!(matcher.find("a.PDF").is_none());
    }

    #[test]
    fn regex_mode() {
        let matcher = m(r"^report_\d{4}", Mode::Regex, false);
        assert_eq!(matcher.find("report_2025.pdf"), Some(0..11));
        assert_eq!(matcher.find("my_report_2025.pdf"), None);
    }

    #[test]
    fn invalid_patterns_are_errors() {
        assert!(Matcher::new("(unclosed", Mode::Regex, false).is_err());
        assert!(Matcher::new("[unclosed", Mode::Glob, false).is_err());
    }
}
