//! English and Turkish UI texts.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Lang {
    En,
    Tr,
}

impl Lang {
    /// Turkish if the system language is Turkish, otherwise English.
    pub fn from_system() -> Self {
        match sys_locale::get_locale() {
            Some(locale) if locale.to_lowercase().starts_with("tr") => Lang::Tr,
            _ => Lang::En,
        }
    }

    pub fn texts(self) -> &'static Texts {
        match self {
            Lang::En => &EN,
            Lang::Tr => &TR,
        }
    }

    /// Formats a number with this language's thousands separator.
    pub fn number(self, n: u64) -> String {
        let sep = match self {
            Lang::En => ',',
            Lang::Tr => '.',
        };
        let digits = n.to_string();
        let mut out = String::with_capacity(digits.len() + digits.len() / 3);
        for (i, c) in digits.chars().enumerate() {
            if i > 0 && (digits.len() - i) % 3 == 0 {
                out.push(sep);
            }
            out.push(c);
        }
        out
    }
}

pub struct Texts {
    pub pattern: &'static str,
    pub pattern_hint: &'static str,
    pub folder: &'static str,
    pub browse: &'static str,
    pub mode: &'static str,
    pub contains: &'static str,
    pub glob: &'static str,
    pub regex: &'static str,
    pub case_sensitive: &'static str,
    pub show: &'static str,
    pub all: &'static str,
    pub files: &'static str,
    pub dirs: &'static str,
    pub hidden: &'static str,
    pub gitignore: &'static str,
    pub extensions: &'static str,
    pub extensions_hint: &'static str,
    pub search: &'static str,
    pub stop: &'static str,
    pub open: &'static str,
    pub show_in_folder: &'static str,
    pub copy_path: &'static str,
    pub searching: &'static str,
    pub results: &'static str,
    pub scanned: &'static str,
    pub skipped: &'static str,
    pub stopped: &'static str,
    pub idle: &'static str,
    pub no_results: &'static str,
    pub folder_missing: &'static str,
    pub hint: &'static str,
}

pub const EN: Texts = Texts {
    pattern: "Search:",
    pattern_hint: "part of a file name, e.g. report",
    folder: "Folder:",
    browse: "📁 Browse…",
    mode: "Match:",
    contains: "Contains",
    glob: "Glob (*.pdf)",
    regex: "Regex",
    case_sensitive: "Match case",
    show: "Show:",
    all: "All",
    files: "Files",
    dirs: "Folders",
    hidden: "Hidden files",
    gitignore: "Respect .gitignore",
    extensions: "Extensions:",
    extensions_hint: "pdf, docx (empty = all)",
    search: "🔍 Search",
    stop: "⏹ Stop",
    open: "Open",
    show_in_folder: "Show in folder",
    copy_path: "Copy path",
    searching: "Searching…",
    results: "results",
    scanned: "entries scanned",
    skipped: "skipped",
    stopped: "stopped",
    idle: "Type a name and press Enter",
    no_results: "No results",
    folder_missing: "Folder not found",
    hint: "Double-click: show in folder · Right-click: more · Esc: stop",
};

pub const TR: Texts = Texts {
    pattern: "Ara:",
    pattern_hint: "dosya adının bir kısmı, ör. rapor",
    folder: "Klasör:",
    browse: "📁 Gözat…",
    mode: "Eşleşme:",
    contains: "İçerir",
    glob: "Glob (*.pdf)",
    regex: "Regex",
    case_sensitive: "Büyük/küçük harf duyarlı",
    show: "Göster:",
    all: "Hepsi",
    files: "Dosyalar",
    dirs: "Klasörler",
    hidden: "Gizli dosyalar",
    gitignore: ".gitignore'a uy",
    extensions: "Uzantılar:",
    extensions_hint: "pdf, docx (boş = hepsi)",
    search: "🔍 Ara",
    stop: "⏹ Durdur",
    open: "Aç",
    show_in_folder: "Klasörde göster",
    copy_path: "Yolu kopyala",
    searching: "Aranıyor…",
    results: "sonuç",
    scanned: "girdi tarandı",
    skipped: "atlandı",
    stopped: "durduruldu",
    idle: "Bir isim yazıp Enter'a basın",
    no_results: "Sonuç yok",
    folder_missing: "Klasör bulunamadı",
    hint: "Çift tık: klasörde göster · Sağ tık: diğer · Esc: durdur",
};

#[cfg(test)]
mod tests {
    use super::Lang;

    #[test]
    fn numbers_use_language_separator() {
        assert_eq!(Lang::En.number(1234567), "1,234,567");
        assert_eq!(Lang::Tr.number(1234567), "1.234.567");
        assert_eq!(Lang::Tr.number(999), "999");
    }
}
