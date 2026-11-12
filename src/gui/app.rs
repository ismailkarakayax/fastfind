use std::ops::Range;
use std::path::PathBuf;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crossbeam_channel::Receiver;
use eframe::egui::{self, text::LayoutJob, Color32, Key, TextFormat, TextStyle, TextWrapMode};
use fastfind::matcher::{Matcher, Mode};
use fastfind::search::{self, FileType, Hit, Options, Stats};
use serde::{Deserialize, Serialize};

use crate::i18n::Lang;
use crate::platform;

/// How many results to move from the channel into the list per frame, so
/// the window stays responsive even when millions of results pour in.
const MAX_ROWS_PER_FRAME: usize = 200_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum MatchMode {
    Contains,
    Glob,
    Regex,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum Kind {
    All,
    Files,
    Dirs,
}

/// Everything the user picked, remembered between launches.
#[derive(Serialize, Deserialize)]
#[serde(default)]
struct Settings {
    pattern: String,
    folder: String,
    mode: MatchMode,
    case_sensitive: bool,
    kind: Kind,
    hidden: bool,
    gitignore: bool,
    extensions: String,
    lang: Lang,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            pattern: String::new(),
            folder: platform::home_dir(),
            mode: MatchMode::Contains,
            case_sensitive: false,
            kind: Kind::All,
            hidden: false,
            gitignore: false,
            extensions: String::new(),
            lang: Lang::from_system(),
        }
    }
}

/// One line in the result list.
struct Row {
    path: String,
    /// Byte offset where the file name starts inside `path`.
    name_start: usize,
    /// Byte range of the match inside `path` (empty if unknown).
    matched: Range<usize>,
}

impl Row {
    fn from_hit(hit: Hit) -> Self {
        let path = hit.path.to_string_lossy().into_owned();
        let name_start = path
            .len()
            .checked_sub(hit.name_len)
            .filter(|&start| path.is_char_boundary(start));
        match name_start {
            Some(start) if hit.name_match.end <= hit.name_len => Row {
                matched: start + hit.name_match.start..start + hit.name_match.end,
                name_start: start,
                path,
            },
            _ => Row {
                name_start: path.len(),
                matched: 0..0,
                path,
            },
        }
    }
}

struct Running {
    stats: Arc<Stats>,
    rx: Receiver<Hit>,
    /// Returns how long the search took.
    handle: JoinHandle<Duration>,
}

enum Status {
    Idle,
    Running,
    Done {
        secs: f64,
        scanned: u64,
        skipped: u64,
        stopped: bool,
    },
    Error(String),
}

enum Action {
    Open(usize),
    ShowInFolder(usize),
    CopyPath(usize),
}

pub struct App {
    settings: Settings,
    rows: Vec<Row>,
    selected: Option<usize>,
    running: Option<Running>,
    status: Status,
    user_stopped: bool,
    focus_pattern: bool,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let settings = cc
            .storage
            .and_then(|storage| eframe::get_value(storage, eframe::APP_KEY))
            .unwrap_or_default();
        Self {
            settings,
            rows: Vec::new(),
            selected: None,
            running: None,
            status: Status::Idle,
            user_stopped: false,
            focus_pattern: true,
        }
    }

    fn start(&mut self, ctx: &egui::Context) {
        self.stop();
        let s = &self.settings;
        let t = s.lang.texts();

        let mode = match s.mode {
            MatchMode::Contains => Mode::Substring,
            MatchMode::Glob => Mode::Glob,
            MatchMode::Regex => Mode::Regex,
        };
        let matcher = match Matcher::new(&s.pattern, mode, s.case_sensitive) {
            Ok(m) => m,
            Err(e) => {
                self.status = Status::Error(e);
                return;
            }
        };

        let folder = PathBuf::from(s.folder.trim());
        if !folder.is_dir() {
            self.status = Status::Error(format!("{}: {}", t.folder_missing, folder.display()));
            return;
        }
        let root = std::path::absolute(&folder).unwrap_or(folder);

        let opts = Options {
            roots: vec![root],
            hidden: s.hidden,
            gitignore: s.gitignore,
            max_depth: None,
            threads: 0,
            file_type: match s.kind {
                Kind::All => None,
                Kind::Files => Some(FileType::File),
                Kind::Dirs => Some(FileType::Dir),
            },
            extensions: search::parse_extensions([s.extensions.as_str()]),
            max_results: None,
        };

        let stats = Arc::new(Stats::default());
        let (tx, rx) = crossbeam_channel::unbounded();
        let worker_stats = Arc::clone(&stats);
        let ctx = ctx.clone();
        let handle = std::thread::spawn(move || {
            let started = Instant::now();
            search::run(&opts, &matcher, &worker_stats, tx);
            ctx.request_repaint();
            started.elapsed()
        });

        self.rows.clear();
        self.selected = None;
        self.user_stopped = false;
        self.running = Some(Running { stats, rx, handle });
        self.status = Status::Running;
    }

    fn stop(&mut self) {
        if let Some(run) = &self.running {
            run.stats.stop.store(true, Relaxed);
            self.user_stopped = true;
        }
    }

    /// Moves new results into the list and notices when the search ends.
    fn poll(&mut self, ctx: &egui::Context) {
        let Some(run) = &self.running else {
            return;
        };
        self.rows.extend(
            run.rx
                .try_iter()
                .take(MAX_ROWS_PER_FRAME)
                .map(Row::from_hit),
        );
        let finished = run.handle.is_finished() && run.rx.is_empty();
        if !finished {
            ctx.request_repaint_after(Duration::from_millis(30));
            return;
        }
        let run = self.running.take().expect("checked above");
        let elapsed = run.handle.join().unwrap_or_default();
        self.status = Status::Done {
            secs: elapsed.as_secs_f64(),
            scanned: run.stats.scanned.load(Relaxed),
            skipped: run.stats.skipped.load(Relaxed),
            stopped: self.user_stopped,
        };
    }

    fn controls(&mut self, ui: &mut egui::Ui) {
        let t = self.settings.lang.texts();
        let mut go = false;
        let enter = |ui: &egui::Ui, r: &egui::Response| {
            r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter))
        };

        ui.add_space(8.0);
        egui::Grid::new("form")
            .num_columns(2)
            .spacing([10.0, 8.0])
            .show(ui, |ui| {
                ui.label(t.pattern);
                ui.horizontal(|ui| {
                    let width = (ui.available_width() - 210.0).max(160.0);
                    let r = ui.add(
                        egui::TextEdit::singleline(&mut self.settings.pattern)
                            .hint_text(t.pattern_hint)
                            .desired_width(width),
                    );
                    if self.focus_pattern {
                        r.request_focus();
                        self.focus_pattern = false;
                    }
                    go |= enter(ui, &r);
                    if self.running.is_some() {
                        if ui.button(t.stop).clicked() {
                            self.stop();
                        }
                    } else if ui.button(t.search).clicked() {
                        go = true;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.selectable_value(&mut self.settings.lang, Lang::Tr, "TR");
                        ui.selectable_value(&mut self.settings.lang, Lang::En, "EN");
                    });
                });
                ui.end_row();

                ui.label(t.folder);
                ui.horizontal(|ui| {
                    let width = (ui.available_width() - 210.0).max(160.0);
                    let r = ui.add(
                        egui::TextEdit::singleline(&mut self.settings.folder).desired_width(width),
                    );
                    go |= enter(ui, &r);
                    if ui.button(t.browse).clicked() {
                        let picked = rfd::FileDialog::new()
                            .set_directory(self.settings.folder.trim())
                            .pick_folder();
                        if let Some(dir) = picked {
                            self.settings.folder = dir.display().to_string();
                        }
                    }
                });
                ui.end_row();

                ui.label(t.mode);
                ui.horizontal(|ui| {
                    let mode = &mut self.settings.mode;
                    ui.radio_value(mode, MatchMode::Contains, t.contains);
                    ui.radio_value(mode, MatchMode::Glob, t.glob);
                    ui.radio_value(mode, MatchMode::Regex, t.regex);
                    ui.separator();
                    ui.checkbox(&mut self.settings.case_sensitive, t.case_sensitive);
                });
                ui.end_row();

                ui.label(t.show);
                ui.horizontal(|ui| {
                    let kind = &mut self.settings.kind;
                    ui.radio_value(kind, Kind::All, t.all);
                    ui.radio_value(kind, Kind::Files, t.files);
                    ui.radio_value(kind, Kind::Dirs, t.dirs);
                    ui.separator();
                    ui.checkbox(&mut self.settings.hidden, t.hidden);
                    ui.checkbox(&mut self.settings.gitignore, t.gitignore);
                });
                ui.end_row();

                ui.label(t.extensions);
                let r = ui.add(
                    egui::TextEdit::singleline(&mut self.settings.extensions)
                        .hint_text(t.extensions_hint)
                        .desired_width(240.0),
                );
                go |= enter(ui, &r);
                ui.end_row();
            });
        ui.add_space(8.0);

        if go {
            self.start(ui.ctx());
        }
    }

    fn status_bar(&self, ui: &mut egui::Ui) {
        let lang = self.settings.lang;
        let t = lang.texts();
        ui.horizontal(|ui| {
            match &self.status {
                Status::Idle => {
                    ui.weak(t.idle);
                }
                Status::Running => {
                    let scanned = self
                        .running
                        .as_ref()
                        .map_or(0, |r| r.stats.scanned.load(Relaxed));
                    ui.spinner();
                    ui.label(format!(
                        "{} {} {} · {} {}",
                        t.searching,
                        lang.number(self.rows.len() as u64),
                        t.results,
                        lang.number(scanned),
                        t.scanned
                    ));
                }
                Status::Done {
                    secs,
                    scanned,
                    skipped,
                    stopped,
                } => {
                    let mut text = if self.rows.is_empty() {
                        t.no_results.to_string()
                    } else {
                        format!("{} {}", lang.number(self.rows.len() as u64), t.results)
                    };
                    text.push_str(&format!(" · {} {}", lang.number(*scanned), t.scanned));
                    if *skipped > 0 {
                        text.push_str(&format!(" · {} {}", lang.number(*skipped), t.skipped));
                    }
                    let secs = format!("{secs:.2}");
                    let secs = match lang {
                        Lang::Tr => secs.replace('.', ","),
                        Lang::En => secs,
                    };
                    text.push_str(&format!(" · {secs} s"));
                    if *stopped {
                        text.push_str(&format!(" ({})", t.stopped));
                    }
                    ui.label(text);
                }
                Status::Error(e) => {
                    ui.colored_label(ui.visuals().error_fg_color, e);
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.weak(t.hint);
            });
        });
    }

    fn results(&mut self, ui: &mut egui::Ui) {
        let t = self.settings.lang.texts();
        if self.rows.is_empty() {
            return;
        }
        let row_height =
            ui.text_style_height(&TextStyle::Body) + 2.0 * ui.spacing().button_padding.y;
        let mut action = None;

        egui::ScrollArea::both().auto_shrink(false).show_rows(
            ui,
            row_height,
            self.rows.len(),
            |ui, range| {
                ui.style_mut().wrap_mode = Some(TextWrapMode::Extend);
                for i in range {
                    let job = row_layout(ui, &self.rows[i]);
                    let r = ui.selectable_label(self.selected == Some(i), job);
                    if r.clicked() {
                        self.selected = Some(i);
                    }
                    if r.double_clicked() {
                        action = Some(Action::ShowInFolder(i));
                    }
                    r.context_menu(|ui| {
                        self.selected = Some(i);
                        if ui.button(t.open).clicked() {
                            action = Some(Action::Open(i));
                            ui.close();
                        }
                        if ui.button(t.show_in_folder).clicked() {
                            action = Some(Action::ShowInFolder(i));
                            ui.close();
                        }
                        if ui.button(t.copy_path).clicked() {
                            action = Some(Action::CopyPath(i));
                            ui.close();
                        }
                    });
                }
            },
        );

        match action {
            Some(Action::Open(i)) => {
                let _ = platform::open(&self.rows[i].path);
            }
            Some(Action::ShowInFolder(i)) => {
                let _ = platform::show_in_folder(&self.rows[i].path);
            }
            Some(Action::CopyPath(i)) => ui.ctx().copy_text(self.rows[i].path.clone()),
            None => {}
        }
    }
}

/// Folder part dimmed, file name bright, matched part highlighted.
fn row_layout(ui: &egui::Ui, row: &Row) -> LayoutJob {
    let font_id = TextStyle::Body.resolve(ui.style());
    let visuals = ui.visuals();
    let dir = TextFormat {
        font_id: font_id.clone(),
        color: visuals.weak_text_color(),
        ..Default::default()
    };
    let name = TextFormat {
        font_id: font_id.clone(),
        color: visuals.strong_text_color(),
        ..Default::default()
    };
    let highlight = TextFormat {
        font_id,
        color: if visuals.dark_mode {
            Color32::from_rgb(255, 190, 70)
        } else {
            Color32::from_rgb(190, 80, 0)
        },
        background: if visuals.dark_mode {
            Color32::from_rgba_unmultiplied(255, 190, 70, 30)
        } else {
            Color32::from_rgba_unmultiplied(255, 160, 0, 45)
        },
        ..Default::default()
    };

    let mut job = LayoutJob::default();
    let path = row.path.as_str();
    job.append(&path[..row.name_start], 0.0, dir);
    let m = &row.matched;
    if m.is_empty() || m.start < row.name_start {
        job.append(&path[row.name_start..], 0.0, name);
    } else {
        job.append(&path[row.name_start..m.start], 0.0, name.clone());
        job.append(&path[m.clone()], 0.0, highlight);
        job.append(&path[m.end..], 0.0, name);
    }
    job
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll(&ctx);
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.stop();
        }

        egui::Panel::top("controls").show(ui, |ui| self.controls(ui));
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.add_space(4.0);
            self.status_bar(ui);
            ui.add_space(4.0);
        });
        egui::CentralPanel::default().show(ui, |ui| self.results(ui));
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.settings);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_offsets_point_into_the_full_path() {
        let row = Row::from_hit(Hit {
            path: PathBuf::from("/home/me/my_report.txt"),
            name_len: "my_report.txt".len(),
            name_match: 3..9,
        });
        assert_eq!(&row.path[row.name_start..], "my_report.txt");
        assert_eq!(&row.path[row.matched.clone()], "report");
    }

    #[test]
    fn row_with_multibyte_name() {
        let row = Row::from_hit(Hit {
            path: PathBuf::from("/belgeler/Şirket_raporu.txt"),
            name_len: "Şirket_raporu.txt".len(),
            name_match: 0..7,
        });
        assert_eq!(&row.path[row.matched.clone()], "Şirket");
    }

    #[test]
    fn row_with_inconsistent_lengths_falls_back_safely() {
        let row = Row::from_hit(Hit {
            path: PathBuf::from("/a.txt"),
            name_len: 99,
            name_match: 0..3,
        });
        assert_eq!(row.name_start, row.path.len());
        assert!(row.matched.is_empty());
    }
}
