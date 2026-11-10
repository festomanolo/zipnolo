use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct FolderItem {
    /// 0 = directory, 1 = regular file, 2 = executable file (Unix chmod +x)
    pub kind: u8,
    pub rel_path: String,
    pub full_path: PathBuf,
    pub file_size: u64,
}

#[cfg(unix)]
pub fn is_executable(meta: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    meta.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
pub fn is_executable(_meta: &fs::Metadata) -> bool {
    false
}

#[cfg(unix)]
pub fn set_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(path)?.permissions();
    let mode = perms.mode();
    perms.set_mode(mode | 0o755);
    fs::set_permissions(path, perms)?;
    Ok(())
}

#[cfg(not(unix))]
pub fn set_executable(_path: &Path) -> Result<()> {
    Ok(())
}

use anyhow::Context;

pub fn collect_folder_items(
    root: &Path,
    current: &Path,
    exclude_archive: Option<&Path>,
    out: &mut Vec<FolderItem>,
) -> Result<()> {
    let mut entries: Vec<_> = fs::read_dir(current)
        .with_context(|| format!("Failed to read directory: {}", current.display()))?
        .flatten()
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name == ".DS_Store" {
            continue;
        }
        if let Some(ex) = exclude_archive {
            if path == ex {
                continue;
            }
        }
        // Use symlink_metadata so broken symlinks or Linux special files (FIFOs/sockets) never fail
        let meta = match fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let file_type = meta.file_type();
        if file_type.is_symlink() {
            continue;
        }
        let rel = path
            .strip_prefix(root)
            .context("Failed to compute relative path")?;
        // Normalize path separators to forward slashes for cross-platform Linux/macOS/Windows portability
        let rel_str = rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");

        if file_type.is_dir() {
            out.push(FolderItem {
                kind: 0,
                rel_path: rel_str,
                full_path: path.clone(),
                file_size: 0,
            });
            collect_folder_items(root, &path, exclude_archive, out)?;
        } else if file_type.is_file() {
            let kind = if is_executable(&meta) { 2 } else { 1 };
            out.push(FolderItem {
                kind,
                rel_path: rel_str,
                full_path: path,
                file_size: meta.len(),
            });
        }
    }
    Ok(())
}
