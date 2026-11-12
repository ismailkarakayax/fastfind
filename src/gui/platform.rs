//! Opening files and folders with the operating system.

use std::io;
use std::path::Path;
use std::process::Command;

/// Opens the file (or folder) with its default application.
pub fn open(path: &str) -> io::Result<()> {
    #[cfg(windows)]
    let mut cmd = {
        let mut c = Command::new("explorer");
        c.arg(path);
        c
    };
    #[cfg(target_os = "macos")]
    let mut cmd = {
        let mut c = Command::new("open");
        c.arg(path);
        c
    };
    #[cfg(not(any(windows, target_os = "macos")))]
    let mut cmd = {
        let mut c = Command::new("xdg-open");
        c.arg(path);
        c
    };
    cmd.spawn().map(drop)
}

/// Opens the containing folder with the entry selected (where supported).
pub fn show_in_folder(path: &str) -> io::Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // explorer needs exactly `/select,"C:\path"`; std's quoting would
        // put quotes around the whole argument instead.
        Command::new("explorer")
            .raw_arg(format!("/select,\"{path}\""))
            .spawn()
            .map(drop)
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg("-R").arg(path).spawn().map(drop)
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let parent = Path::new(path).parent().unwrap_or(Path::new("/"));
        Command::new("xdg-open").arg(parent).spawn().map(drop)
    }
}

/// The user's home folder, used as the default search location.
pub fn home_dir() -> String {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(|p| Path::new(&p).display().to_string())
        .unwrap_or_else(|| ".".to_string())
}
