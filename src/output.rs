use std::io::{self, Write};

use anstyle::{AnsiColor, Style};

use fastfind::search::Hit;

const DIR_STYLE: Style = Style::new().fg_color(Some(anstyle::Color::Ansi(AnsiColor::Blue)));
const MATCH_STYLE: Style = Style::new()
    .bold()
    .fg_color(Some(anstyle::Color::Ansi(AnsiColor::BrightRed)));

pub struct Printer<W: Write> {
    out: W,
    color: bool,
}

impl<W: Write> Printer<W> {
    pub fn new(out: W, color: bool) -> Self {
        Self { out, color }
    }

    pub fn print(&mut self, hit: &Hit) -> io::Result<()> {
        let path = hit.path.to_string_lossy();
        let path: &str = &path;

        let name_start = path.len().wrapping_sub(hit.name_len);
        let highlightable = self.color
            && hit.name_len <= path.len()
            && path.is_char_boundary(name_start)
            && hit.name_match.end <= hit.name_len;

        if !highlightable {
            self.out.write_all(path.as_bytes())?;
            return self.out.write_all(b"\n");
        }

        let (dir, name) = path.split_at(name_start);
        let (before, rest) = name.split_at(hit.name_match.start);
        let (matched, after) = rest.split_at(hit.name_match.len());
        writeln!(
            self.out,
            "{DIR_STYLE}{dir}{DIR_STYLE:#}{before}{MATCH_STYLE}{matched}{MATCH_STYLE:#}{after}"
        )
    }

    pub fn flush(&mut self) -> io::Result<()> {
        self.out.flush()
    }
}

/// Formats a number with thousands separators: 1234567 -> "1,234,567".
pub fn group_digits(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn hit(path: &str, name_len: usize, range: std::ops::Range<usize>) -> Hit {
        Hit {
            path: PathBuf::from(path),
            name_len,
            name_match: range,
        }
    }

    #[test]
    fn plain_output() {
        let mut p = Printer::new(Vec::new(), false);
        p.print(&hit("/a/b/report.txt", 10, 0..6)).unwrap();
        assert_eq!(String::from_utf8(p.out).unwrap(), "/a/b/report.txt\n");
    }

    #[test]
    fn colored_output_highlights_match() {
        let mut p = Printer::new(Vec::new(), true);
        p.print(&hit("/a/my_report.txt", 13, 3..9)).unwrap();
        let s = String::from_utf8(p.out).unwrap();
        let expected =
            format!("{DIR_STYLE}/a/{DIR_STYLE:#}my_{MATCH_STYLE}report{MATCH_STYLE:#}.txt\n");
        assert_eq!(s, expected);
    }

    #[test]
    fn groups_digits() {
        assert_eq!(group_digits(0), "0");
        assert_eq!(group_digits(999), "999");
        assert_eq!(group_digits(1000), "1,000");
        assert_eq!(group_digits(1234567), "1,234,567");
    }
}
