use anyhow::Result;
use crossterm::{
    cursor,
    event::{
        KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute,
    style::ResetColor,
    terminal,
};
use std::io::{self, Write};
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub enum NavEntryKind {
    SelectCurrentFolder,
    CompressCurrentFolder,
    ParentDir,
    Directory(PathBuf),
    File(PathBuf),
}

#[derive(Clone, Debug)]
pub struct NavEntry {
    pub label: String,
    pub kind: NavEntryKind,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BrowseMode {
    SelectFileOrFolderToCompress,
    SelectDestinationFolder,
}

pub struct RawModeGuard {
    enhanced_keys: bool,
}

impl RawModeGuard {
    pub fn acquire() -> Result<Self> {
        terminal::enable_raw_mode()?;
        let enhanced_keys = execute!(
            io::stdout(),
            PushKeyboardEnhancementFlags(
                KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
            )
        )
        .is_ok();
        execute!(io::stdout(), cursor::Hide)?;
        Ok(Self { enhanced_keys })
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        if self.enhanced_keys {
            let _ = execute!(io::stdout(), PopKeyboardEnhancementFlags);
        }
        let _ = execute!(io::stdout(), cursor::Show, ResetColor);
        let _ = terminal::disable_raw_mode();
    }
}

pub fn prompt_line(prompt: &str) -> Result<String> {
    print!("{}", prompt);
    io::stdout().flush()?;
    let mut buf = String::new();
    io::stdin().read_line(&mut buf)?;
    Ok(buf.trim().to_string())
}

use crate::constants::COMPRESS_KEY_LABEL;
use crate::paths::is_nolo_archive;
use crate::progress::format_bytes;
use std::fs;
use std::path::Path;

pub fn load_nav_entries(dir: &Path, mode: BrowseMode) -> Result<Vec<NavEntry>> {
    let mut entries = Vec::new();
    let folder_name = dir
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("current_folder");

    if mode == BrowseMode::SelectDestinationFolder {
        entries.push(NavEntry {
            label: format!("📌 [Select THIS Destination Folder: {}]", dir.display()),
            kind: NavEntryKind::SelectCurrentFolder,
        });
    } else {
        entries.push(NavEntry {
            label: format!(
                "📦 [Compress THIS Entire Folder: '{}/'] ({} or Enter)",
                folder_name, COMPRESS_KEY_LABEL
            ),
            kind: NavEntryKind::CompressCurrentFolder,
        });
    }

    if dir.parent().is_some() {
        entries.push(NavEntry {
            label: "📁 .. (Parent Directory)".to_string(),
            kind: NavEntryKind::ParentDir,
        });
    }

    let mut dirs = Vec::new();
    let mut files = Vec::new();

    if let Ok(read_dir) = fs::read_dir(dir) {
        for item in read_dir.flatten() {
            let path = item.path();
            let name = item.file_name().to_string_lossy().to_string();
            if name == ".DS_Store" {
                continue;
            }
            if let Ok(meta) = fs::symlink_metadata(&path) {
                if meta.is_dir() {
                    dirs.push((name, path));
                } else if mode == BrowseMode::SelectFileOrFolderToCompress && meta.is_file() {
                    files.push((name, path, meta.len()));
                }
            }
        }
    }

    dirs.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    files.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));

    for (name, path) in dirs {
        let hint = if mode == BrowseMode::SelectFileOrFolderToCompress {
            format!("  (Enter: Open | {}: Compress Folder)", COMPRESS_KEY_LABEL)
        } else {
            String::new()
        };
        entries.push(NavEntry {
            label: format!("📁 {}/{}", name, hint),
            kind: NavEntryKind::Directory(path),
        });
    }

    for (name, path, size) in files {
        let icon = if is_nolo_archive(&path) { "📦" } else { "📄" };
        entries.push(NavEntry {
            label: format!("{} {:<38} ({})", icon, name, format_bytes(size)),
            kind: NavEntryKind::File(path),
        });
    }

    Ok(entries)
}
