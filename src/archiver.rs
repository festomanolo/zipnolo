use crate::algorithm::Algorithm;
use crate::codec::compress_chunk;
use crate::constants::START_MAGIC;
use crate::format::{write_nolo_footer, BlockEntry, CompressedBlock};
use crate::progress::ProgressBar;
use anyhow::{bail, Context, Result};
use crossbeam_channel::bounded;
use memmap2::Mmap;
use rayon::prelude::*;
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;
use xxhash_rust::xxh3::xxh3_64;

pub fn compress_file(
    input_path: &Path,
    output_path: &Path,
    algo: Algorithm,
    chunk_size: usize,
) -> Result<()> {
    compress_file_with_progress(input_path, output_path, algo, chunk_size, false)
}

pub fn compress_file_with_progress(
    input_path: &Path,
    output_path: &Path,
    algo: Algorithm,
    chunk_size: usize,
    show_progress: bool,
) -> Result<()> {
    if chunk_size == 0 {
        bail!("Chunk size must be greater than 0");
    }

    let in_file = File::open(input_path)
        .with_context(|| format!("Failed to open input file: {}", input_path.display()))?;
    let file_size = in_file.metadata()?.len();

    // Cross-platform path normalization: store purely as a UTF-8 .file_name() without directory prefixes
    let file_name = input_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown_file")
        .as_bytes();

    if let Some(parent) = output_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create destination folder: {}", parent.display())
            })?;
        }
    }

    let out_file = File::create(output_path)
        .with_context(|| format!("Failed to create output archive: {}", output_path.display()))?;
    let mut writer = BufWriter::with_capacity(8 * 1024 * 1024, out_file);

    // 1. Write Start Magic
    writer.write_all(START_MAGIC)?;

    let mut block_table: Vec<BlockEntry> = Vec::new();
    let progress = ProgressBar::new("Compressing", file_size, show_progress);

    // Cross-platform safety: Only memory-map if file_size > 0
    if file_size > 0 {
        let mmap = unsafe { Mmap::map(&in_file).context("Failed to memory-map input file")? };
        let num_blocks = mmap.len().div_ceil(chunk_size);
        block_table.reserve(num_blocks);

        // Bounded channel limits memory usage to (2 * CPU cores) blocks in flight
        let queue_cap = (rayon::current_num_threads() * 2).max(4);
        let (tx, rx) = bounded::<Result<CompressedBlock>>(queue_cap);

        std::thread::scope(|s| -> Result<()> {
            s.spawn(|| {
                mmap.par_chunks(chunk_size)
                    .enumerate()
                    .for_each_with(tx, |sender, (index, chunk)| {
                        let checksum = xxh3_64(chunk);
                        let res = compress_chunk(chunk, algo).map(|data| CompressedBlock {
                            index,
                            uncompressed_size: chunk.len(),
                            data,
                            checksum,
                        });
                        let _ = sender.send(res);
                    });
            });

            let mut next_expected = 0usize;
            let mut reorder_buffer: BTreeMap<usize, CompressedBlock> = BTreeMap::new();

            for msg in rx {
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

        // Explicitly drop Mmap before closing in_file for Windows file-locking compatibility
        drop(mmap);
    }

    write_nolo_footer(
        &mut writer,
        file_size,
        chunk_size,
        algo,
        file_name,
        &block_table,
    )?;
    progress.finish();

    Ok(())
}
