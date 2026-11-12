//! fastfind desktop app, built with egui.
//!
//! Run with: `cargo run --release --features gui --bin fastfind-gui`

// No console window behind the app in release builds on Windows.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod i18n;
mod platform;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("fastfind")
            .with_inner_size([980.0, 660.0])
            .with_min_inner_size([640.0, 420.0]),
        ..Default::default()
    };
    eframe::run_native(
        "fastfind",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    )
}
