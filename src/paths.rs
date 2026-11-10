use crate::format::read_archive_footer;
use anyhow::{Context, Result};
use std::fs::{self, File};
use std::path::{Path, PathBuf};

/// Returns the default destination directory (the parent directory containing the source file or folder).
pub fn default_same_dir(source_path: &Path) -> PathBuf {
    let resolved = fs::canonicalize(source_path).unwrap_or_else(|_| source_path.to_path_buf());
    match resolved.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    }
}

/// Resolves the output archive file path given an input file/folder and a destination folder (or explicit `.nolo` path).
pub fn resolve_compress_destination(input_path: &Path, dest: &Path) -> Result<PathBuf> {
    let resolved_in = fs::canonicalize(input_path).unwrap_or_else(|_| input_path.to_path_buf());
    let file_name = resolved_in
        .file_name()
        .and_then(|n| n.to_str())
        .context("Input path does not have a valid UTF-8 name")?;

    let is_explicit_nolo_file = !dest.is_dir()
        && dest
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("nolo"))
            .unwrap_or(false);

    if is_explicit_nolo_file {
        if let Some(parent) = dest.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }
        Ok(dest.to_path_buf())
    } else {
        fs::create_dir_all(dest).with_context(|| {
            format!("Failed to create destination folder: {}", dest.display())
        })?;
        Ok(dest.join(format!("{}.nolo", file_name)))
    }
}

/// Resolves the output restored file or folder path given an archive file and a destination folder.
pub fn resolve_decompress_destination(archive_path: &Path, dest: &Path) -> Result<PathBuf> {
    let mut archive_file = File::open(archive_path)
        .with_context(|| format!("Failed to open archive: {}", archive_path.display()))?;
    let footer = read_archive_footer(&mut archive_file)?;

    if footer.is_folder {
        fs::create_dir_all(dest).with_context(|| {
            format!("Failed to create destination folder: {}", dest.display())
        })?;
        return Ok(dest.join(footer.stored_name));
    }

    let dest_str = dest.to_string_lossy();
    let looks_like_dir = dest.is_dir()
        || dest_str.ends_with('/')
        || dest_str.ends_with('\\')
        || dest.extension().is_none();

    if looks_like_dir {
        fs::create_dir_all(dest).with_context(|| {
            format!("Failed to create destination folder: {}", dest.display())
        })?;
        Ok(dest.join(footer.stored_name))
    } else {
        if let Some(parent) = dest.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }
        Ok(dest.to_path_buf())
    }
}
