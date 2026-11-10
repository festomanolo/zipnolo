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

use crate::paths::clean_input_path;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    style::{Color, Print, SetForegroundColor},
    terminal::{Clear, ClearType},
};
use std::io::IsTerminal;

/// Interactive terminal navigator using arrow keys, Cmd+Enter / Ctrl+Enter / Alt+Enter / c to compress file/folder, or path pasting.
pub fn interactive_navigator(start_dir: &Path, mode: BrowseMode, title: &str) -> Result<PathBuf> {
    let mut current_dir =
        fs::canonicalize(start_dir).unwrap_or_else(|_| start_dir.to_path_buf());

    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        let prompt = match mode {
            BrowseMode::SelectFileOrFolderToCompress => "Enter source file or folder path: ",
            BrowseMode::SelectDestinationFolder => "Enter destination folder path: ",
        };
        let raw = prompt_line(prompt)?;
        return Ok(clean_input_path(&raw));
    }

    let mut selected_idx: usize = 0;
    let viewport_height: usize = 14;

    loop {
        let entries = load_nav_entries(&current_dir, mode)?;
        if selected_idx >= entries.len() {
            selected_idx = entries.len().saturating_sub(1);
        }

        let start_idx = if selected_idx >= viewport_height {
            selected_idx + 1 - viewport_height
        } else {
            0
        };
        let end_idx = (start_idx + viewport_height).min(entries.len());

        {
            let _guard = RawModeGuard::acquire()?;
            let mut stdout = io::stdout();

            execute!(
                stdout,
                Clear(ClearType::All),
                cursor::MoveTo(0, 0),
                SetForegroundColor(Color::Cyan),
                Print(format!(
                    "=== ZIPNOLO v0.2.0 ({} CPU threads) — Created by festomanolo (https://festomanolo.com) ===\r\n",
                    rayon::current_num_threads()
                )),
                Print(format!("    {}\r\n", title)),
                ResetColor,
                SetForegroundColor(Color::Yellow),
                Print(format!("📂 Location: {}\r\n", current_dir.display())),
                ResetColor,
                SetForegroundColor(Color::DarkGrey),
                Print(format!(
                    "Controls: [↑/↓] Move  [Enter/→] Open Folder  [{}] Compress File/Folder  [←/Bksp] Up Dir  [p] Paste Path  [q] Quit\r\n",
                    COMPRESS_KEY_LABEL
                )),
                Print("-------------------------------------------------------------------------------------------------------------------------------------\r\n"),
                ResetColor
            )?;

            for (idx, entry) in entries
                .iter()
                .enumerate()
                .skip(start_idx)
                .take(end_idx - start_idx)
            {
                if idx == selected_idx {
                    execute!(
                        stdout,
                        SetForegroundColor(Color::Green),
                        Print(format!("  ❯ {}\r\n", entry.label)),
                        ResetColor
                    )?;
                } else {
                    execute!(stdout, Print(format!("    {}\r\n", entry.label)))?;
                }
            }

            if entries.len() > viewport_height {
                execute!(
                    stdout,
                    SetForegroundColor(Color::DarkGrey),
                    Print(format!(
                        "-------------------------------------------------------------------------------------------------------------------------------------\r\nShowing {}-{} of {} items\r\n",
                        start_idx + 1,
                        end_idx,
                        entries.len()
                    )),
                    ResetColor
                )?;
            }
            stdout.flush()?;

            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                let is_compress_shortcut = (key.code == KeyCode::Enter
                    && (key.modifiers.contains(KeyModifiers::SUPER)
                        || key.modifiers.contains(KeyModifiers::META)
                        || key.modifiers.contains(KeyModifiers::ALT)
                        || key.modifiers.contains(KeyModifiers::CONTROL)
                        || key.modifiers.contains(KeyModifiers::SHIFT)))
                    || (key.code == KeyCode::Char('c')
                        && !key.modifiers.contains(KeyModifiers::CONTROL))
                    || key.code == KeyCode::Char(' ');

                if is_compress_shortcut {
                    if let Some(entry) = entries.get(selected_idx) {
                        match &entry.kind {
                            NavEntryKind::SelectCurrentFolder
                            | NavEntryKind::CompressCurrentFolder => {
                                drop(_guard);
                                return Ok(current_dir);
                            }
                            NavEntryKind::Directory(dir_path) => {
                                drop(_guard);
                                return Ok(fs::canonicalize(dir_path)
                                    .unwrap_or_else(|_| dir_path.clone()));
                            }
                            NavEntryKind::File(file_path) => {
                                drop(_guard);
                                return Ok(file_path.clone());
                            }
                            NavEntryKind::ParentDir => {
                                if let Some(parent) = current_dir.parent() {
                                    drop(_guard);
                                    return Ok(parent.to_path_buf());
                                }
                            }
                        }
                    }
                }

                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => {
                        drop(_guard);
                        println!("\nCancelled.");
                        std::process::exit(0);
                    }
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        drop(_guard);
                        println!("\nCancelled.");
                        std::process::exit(0);
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        if selected_idx > 0 {
                            selected_idx -= 1;
                        } else {
                            selected_idx = entries.len().saturating_sub(1);
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if selected_idx + 1 < entries.len() {
                            selected_idx += 1;
                        } else {
                            selected_idx = 0;
                        }
                    }
                    KeyCode::Backspace | KeyCode::Left => {
                        if let Some(parent) = current_dir.parent() {
                            current_dir = parent.to_path_buf();
                            selected_idx = 0;
                        }
                    }
                    KeyCode::Char('p') | KeyCode::Char('/') => {
                        drop(_guard);
                        println!();
                        let raw = prompt_line(
                            "Paste file or folder path (or press Enter to cancel): ",
                        )?;
                        if !raw.is_empty() {
                            let pasted = clean_input_path(&raw);
                            if pasted.is_dir() {
                                if mode == BrowseMode::SelectFileOrFolderToCompress {
                                    let action = prompt_line(
                                        "Press [Enter] to browse inside this folder, or [c] to compress the entire folder now: ",
                                    )?;
                                    if action.eq_ignore_ascii_case("c") {
                                        return Ok(
                                            fs::canonicalize(&pasted).unwrap_or(pasted)
                                        );
                                    }
                                }
                                current_dir = fs::canonicalize(&pasted).unwrap_or(pasted);
                                selected_idx = 0;
                            } else if pasted.is_file() {
                                if mode == BrowseMode::SelectFileOrFolderToCompress {
                                    return Ok(pasted);
                                } else if let Some(parent) = pasted.parent() {
                                    return Ok(parent.to_path_buf());
                                }
                            } else if mode == BrowseMode::SelectDestinationFolder {
                                fs::create_dir_all(&pasted)?;
                                return Ok(pasted);
                            } else {
                                println!("Path does not exist: {}", pasted.display());
                                let _ = prompt_line("Press Enter to continue...");
                            }
                        }
                    }
                    KeyCode::Enter | KeyCode::Right => {
                        if let Some(entry) = entries.get(selected_idx) {
                            match &entry.kind {
                                NavEntryKind::SelectCurrentFolder
                                | NavEntryKind::CompressCurrentFolder => {
                                    drop(_guard);
                                    return Ok(current_dir);
                                }
                                NavEntryKind::ParentDir => {
                                    if let Some(parent) = current_dir.parent() {
                                        current_dir = parent.to_path_buf();
                                        selected_idx = 0;
                                    }
                                }
                                NavEntryKind::Directory(dir_path) => {
                                    current_dir = fs::canonicalize(dir_path)
                                        .unwrap_or_else(|_| dir_path.clone());
                                    selected_idx = 0;
                                }
                                NavEntryKind::File(file_path) => {
                                    if key.code == KeyCode::Enter {
                                        drop(_guard);
                                        return Ok(file_path.clone());
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}
