use crate::algorithm::Algorithm;
use crate::codec::{compress_chunk, decompress_chunk_into};
use crate::constants::START_MAGIC;
use crate::folder::{prepare_overwrite_path, unpack_folder_stream, ParallelBlockReader};
use crate::format::{read_archive_footer, write_nolo_footer, BlockEntry, CompressedBlock};
use crate::progress::ProgressBar;
use anyhow::{bail, Context, Result};
use crossbeam_channel::bounded;
use memmap2::{Mmap, MmapMut};
use rayon::prelude::*;
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
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

    prepare_overwrite_path(output_path);
    let out_file = File::create(output_path)
        .with_context(|| format!("Failed to create output archive: {}", output_path.display()))?;
    let mut writer = BufWriter::with_capacity(8 * 1024 * 1024, out_file);
    writer.write_all(START_MAGIC)?;

    let mut block_table: Vec<BlockEntry> = Vec::new();
    let progress = ProgressBar::new("Compressing", file_size, show_progress);

    if file_size > 0 {
        let mmap = unsafe { Mmap::map(&in_file).context("Failed to memory-map input file")? };
        let num_blocks = mmap.len().div_ceil(chunk_size);
        block_table.reserve(num_blocks);

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

pub fn decompress_file(archive_path: &Path, output_path: &Path) -> Result<()> {
    decompress_file_with_progress(archive_path, output_path, false)
}

pub fn decompress_file_with_progress(
    archive_path: &Path,
    output_path: &Path,
    show_progress: bool,
) -> Result<()> {
    let mut archive_file = File::open(archive_path)
        .with_context(|| format!("Failed to open archive: {}", archive_path.display()))?;
    let parsed = read_archive_footer(&mut archive_file)?;

    if parsed.is_folder {
        let in_mmap = unsafe { Mmap::map(&archive_file)? };
        let queue_cap = (rayon::current_num_threads() * 2).max(4);
        let (tx, rx) = bounded::<Result<(usize, Vec<u8>)>>(queue_cap);
        let progress = ProgressBar::new("Decompressing", parsed.orig_size, show_progress);

        std::thread::scope(|s| -> Result<()> {
            let block_meta = &parsed.block_meta;
            let orig_size = parsed.orig_size as usize;
            let chunk_size = parsed.chunk_size;
            let algo = parsed.algo;
            let in_mmap_ref = &in_mmap;

            s.spawn(move || {
                block_meta.par_iter().enumerate().for_each_with(
                    tx,
                    |sender, (i, &(offset, comp_size, expected_hash))| {
                        let res = (|| -> Result<(usize, Vec<u8>)> {
                            let end_offset = offset
                                .checked_add(comp_size)
                                .filter(|&end| end <= in_mmap_ref.len())
                                .context("Compressed block offset out of bounds")?;
                            let compressed_slice = &in_mmap_ref[offset..end_offset];

                            let expected_uncomp_len = if (i + 1) * chunk_size > orig_size {
                                orig_size.saturating_sub(i * chunk_size)
                            } else {
                                chunk_size
                            };
                            let mut out_buf = vec![0u8; expected_uncomp_len];
                            decompress_chunk_into(compressed_slice, &mut out_buf, algo)?;

                            let actual_hash = xxh3_64(&out_buf);
                            if actual_hash != expected_hash {
                                bail!("Block {} failed XXH3 checksum verification!", i);
                            }
                            Ok((i, out_buf))
                        })();
                        let _ = sender.send(res);
                    },
                );
            });

            let mut reader = ParallelBlockReader {
                rx,
                reorder: BTreeMap::new(),
                next_idx: 0,
                current_buf: Vec::new(),
                current_pos: 0,
                progress,
            };
            unpack_folder_stream(&mut reader, output_path)?;
            reader.progress.finish();
            Ok(())
        })?;

        drop(in_mmap);
        return Ok(());
    }

    if let Some(parent) = output_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create destination folder: {}", parent.display())
            })?;
        }
    }

    prepare_overwrite_path(output_path);
    let out_file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(output_path)
        .with_context(|| format!("Failed to create output file: {}", output_path.display()))?;

    let progress = ProgressBar::new("Decompressing", parsed.orig_size, show_progress);

    if parsed.orig_size == 0 {
        progress.finish();
        return Ok(());
    }
    out_file.set_len(parsed.orig_size)?;

    let in_mmap = unsafe { Mmap::map(&archive_file)? };
    let mut out_mmap = unsafe { MmapMut::map_mut(&out_file)? };

    out_mmap
        .par_chunks_mut(parsed.chunk_size)
        .enumerate()
        .try_for_each(|(i, out_chunk)| -> Result<()> {
            let (offset, comp_size, expected_hash) = parsed.block_meta[i];
            let end_offset = offset
                .checked_add(comp_size)
                .filter(|&end| end <= in_mmap.len())
                .context("Compressed block offset out of bounds")?;
            let compressed_slice = &in_mmap[offset..end_offset];

            decompress_chunk_into(compressed_slice, out_chunk, parsed.algo)?;

            let actual_hash = xxh3_64(out_chunk);
            if actual_hash != expected_hash {
                bail!("Block {} failed XXH3 checksum verification!", i);
            }
            progress.add(out_chunk.len() as u64);
            Ok(())
        })?;

    out_mmap.flush()?;
    drop(out_mmap);
    drop(in_mmap);
    progress.finish();
    Ok(())
}
