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

use crate::algorithm::Algorithm;
use crate::codec::compress_chunk;
use crate::constants::{FOLDER_CONTAINER_MAGIC, FOLDER_NAME_PREFIX, START_MAGIC};
use crate::format::{write_nolo_footer, BlockEntry, CompressedBlock};
use crate::progress::ProgressBar;
use anyhow::bail;
use crossbeam_channel::bounded;
use rayon::prelude::*;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufWriter, Read, Write};
use xxhash_rust::xxh3::xxh3_64;

/// Compresses an entire directory tree into a single `.nolo` archive using bounded streaming parallel blocks.
pub fn compress_folder_with_progress(
    folder_path: &Path,
    output_path: &Path,
    algo: Algorithm,
    chunk_size: usize,
    show_progress: bool,
) -> Result<u64> {
    if chunk_size == 0 {
        bail!("Chunk size must be greater than 0");
    }

    let canonical_folder =
        fs::canonicalize(folder_path).unwrap_or_else(|_| folder_path.to_path_buf());
    let folder_name = canonical_folder
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("folder");
    let stored_tag = format!("{}{}", FOLDER_NAME_PREFIX, folder_name);

    if let Some(parent) = output_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }

    let canonical_out = fs::canonicalize(output_path).ok();
    let mut items = Vec::new();
    collect_folder_items(
        &canonical_folder,
        &canonical_folder,
        canonical_out.as_deref(),
        &mut items,
    )?;

    let mut total_stream_size: u64 = 4 + 8; // "NLDR" + item_count (u64)
    for item in &items {
        total_stream_size += 1 + 4 + (item.rel_path.len() as u64) + 8 + item.file_size;
    }

    let out_file = File::create(output_path)
        .with_context(|| format!("Failed to create output archive: {}", output_path.display()))?;
    let mut writer = BufWriter::with_capacity(8 * 1024 * 1024, out_file);
    writer.write_all(START_MAGIC)?;

    let mut block_table: Vec<BlockEntry> = Vec::new();
    let progress = ProgressBar::new("Compressing", total_stream_size, show_progress);

    let queue_cap = (rayon::current_num_threads() * 2).max(4);
    let (raw_tx, raw_rx) = bounded::<Result<(usize, Vec<u8>)>>(queue_cap);
    let (comp_tx, comp_rx) = bounded::<Result<CompressedBlock>>(queue_cap);

    std::thread::scope(|s| -> Result<()> {
        let items_ref = &items;
        s.spawn(move || {
            let mut buf = Vec::with_capacity(chunk_size);
            let mut chunk_idx = 0usize;

            let mut push_bytes = |mut slice: &[u8]| -> bool {
                while !slice.is_empty() {
                    let space = chunk_size - buf.len();
                    let to_copy = slice.len().min(space);
                    buf.extend_from_slice(&slice[..to_copy]);
                    slice = &slice[to_copy..];
                    if buf.len() == chunk_size {
                        let full_chunk =
                            std::mem::replace(&mut buf, Vec::with_capacity(chunk_size));
                        if raw_tx.send(Ok((chunk_idx, full_chunk))).is_err() {
                            return false;
                        }
                        chunk_idx += 1;
                    }
                }
                true
            };

            if !push_bytes(FOLDER_CONTAINER_MAGIC) {
                return;
            }
            if !push_bytes(&(items_ref.len() as u64).to_le_bytes()) {
                return;
            }

            let mut read_buf = vec![0u8; 256 * 1024];
            for item in items_ref {
                let path_bytes = item.rel_path.as_bytes();
                if !push_bytes(&[item.kind])
                    || !push_bytes(&(path_bytes.len() as u32).to_le_bytes())
                    || !push_bytes(path_bytes)
                    || !push_bytes(&item.file_size.to_le_bytes())
                {
                    return;
                }

                if item.kind != 0 && item.file_size > 0 {
                    let mut f = match File::open(&item.full_path) {
                        Ok(f) => f,
                        Err(e) => {
                            let _ = raw_tx.send(Err(anyhow::Error::new(e).context(format!(
                                "Failed to read file {}",
                                item.full_path.display()
                            ))));
                            return;
                        }
                    };
                    let mut remaining = item.file_size;
                    while remaining > 0 {
                        let to_read = (remaining as usize).min(read_buf.len());
                        if let Err(e) = f.read_exact(&mut read_buf[..to_read]) {
                            let _ = raw_tx.send(Err(anyhow::Error::new(e)));
                            return;
                        }
                        if !push_bytes(&read_buf[..to_read]) {
                            return;
                        }
                        remaining -= to_read as u64;
                    }
                }
            }

            if !buf.is_empty() {
                let _ = raw_tx.send(Ok((chunk_idx, buf)));
            }
        });

        s.spawn(move || {
            raw_rx
                .into_iter()
                .par_bridge()
                .for_each_with(comp_tx, |sender, item| {
                    let res = item.and_then(|(index, chunk)| {
                        let checksum = xxh3_64(&chunk);
                        let uncompressed_size = chunk.len();
                        let data = compress_chunk(&chunk, algo)?;
                        Ok(CompressedBlock {
                            index,
                            uncompressed_size,
                            data,
                            checksum,
                        })
                    });
                    let _ = sender.send(res);
                });
        });

        let mut next_expected = 0usize;
        let mut reorder_buffer: BTreeMap<usize, CompressedBlock> = BTreeMap::new();

        for msg in comp_rx {
            let block = msg?;
            reorder_buffer.insert(block.index, block);

            while let Some(ready) = reorder_buffer.remove(&next_expected) {
                writer.write_all(&ready.data)?;
                block_table.push(BlockEntry {
                    compressed_size: ready.data.len() as u32,
                    checksum: ready.checksum,
                });
                progress.add(ready.uncompressed_size as u64);
                next_expected += 1;
            }
        }
        Ok(())
    })?;

    write_nolo_footer(
        &mut writer,
        total_stream_size,
        chunk_size,
        algo,
        stored_tag.as_bytes(),
        &block_table,
    )?;
    progress.finish();

    Ok(total_stream_size)
}
