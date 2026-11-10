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
