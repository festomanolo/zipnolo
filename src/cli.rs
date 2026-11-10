use crate::algorithm::Algorithm;
use crate::archiver::{compress_file_with_progress, decompress_file_with_progress};
use crate::constants::{COMPRESS_KEY_LABEL, DEFAULT_CHUNK_SIZE};
use crate::folder::compress_folder_with_progress;
use crate::format::read_archive_footer;
use crate::navigator::{interactive_navigator, prompt_line, BrowseMode};
use crate::paths::{
    clean_input_path, default_same_dir, is_nolo_archive, resolve_compress_destination,
    resolve_decompress_destination,
};
use crate::progress::format_bytes;
use anyhow::Result;
use std::fs::{self, File};
use std::path::Path;
use std::time::Instant;

pub fn execute_compress_flow(
    src_path: &Path,
    dest_dir: &Path,
    algo: Algorithm,
    chunk_size: usize,
) -> Result<()> {
    let out_archive = resolve_compress_destination(src_path, dest_dir)?;
    let is_dir = src_path.is_dir();

    println!(
        "\nCompressing {} '{}' -> '{}' using {} ({} CPU threads)...",
        if is_dir { "folder" } else { "file" },
        src_path.display(),
        out_archive.display(),
        algo.name(),
        rayon::current_num_threads()
    );

    let start = Instant::now();
    let orig_len = if is_dir {
        compress_folder_with_progress(src_path, &out_archive, algo, chunk_size, true)?
    } else {
        compress_file_with_progress(src_path, &out_archive, algo, chunk_size, true)?;
        fs::metadata(src_path)?.len()
    };
    let elapsed = start.elapsed().as_secs_f64();

    let comp_len = fs::metadata(&out_archive)?.len();
    let ratio = if orig_len > 0 {
        (comp_len as f64 / orig_len as f64) * 100.0
    } else {
        100.0
    };
    let saved_pct = (100.0 - ratio).max(0.0);
    println!(
        "✓ Completed in {:.2}s | {} -> {} ({:.1}% of original, {:.1}% space saved)\n  Saved to: {}",
        elapsed,
        format_bytes(orig_len),
        format_bytes(comp_len),
        ratio,
        saved_pct,
        out_archive.display()
    );
    Ok(())
}

pub fn execute_decompress_flow(archive_path: &Path, dest_dir: &Path) -> Result<()> {
    let out_target = resolve_decompress_destination(archive_path, dest_dir)?;
    println!(
        "\nDecompressing '{}' -> '{}' ({} CPU threads)...",
        archive_path.display(),
        out_target.display(),
        rayon::current_num_threads()
    );

    let start = Instant::now();
    decompress_file_with_progress(archive_path, &out_target, true)?;
    let elapsed = start.elapsed().as_secs_f64();

    let mut archive_file = File::open(archive_path)?;
    let footer = read_archive_footer(&mut archive_file)?;

    println!(
        "✓ Completed in {:.2}s | Restored {} ({}, all XXH3 block checksums verified)\n  Saved to: {}",
        elapsed,
        format_bytes(footer.orig_size),
        if footer.is_folder { "folder" } else { "file" },
        out_target.display()
    );
    Ok(())
}

pub fn run_interactive_from(start_dir: &Path) -> Result<()> {
    let selected_path = interactive_navigator(
        start_dir,
        BrowseMode::SelectFileOrFolderToCompress,
        &format!(
            "Navigate & Press {} to Compress File/Folder (or Select .nolo to Decompress)",
            COMPRESS_KEY_LABEL
        ),
    )?;

    let same_dir = default_same_dir(&selected_path);
    let is_archive = is_nolo_archive(&selected_path);
    let is_folder = selected_path.is_dir();

    println!("\n==========================================================");
    println!(
        "  Selected {} : {}",
        if is_folder { "Folder" } else { "File  " },
        selected_path.display()
    );
    println!(
        "  Action          : {}",
        if is_archive {
            "Decompress (.nolo archive)"
        } else if is_folder {
            "Compress entire folder to .nolo archive"
        } else {
            "Compress file to .nolo archive"
        }
    );
    println!(
        "  Default Dest    : {} (same directory)",
        same_dir.display()
    );
    println!("==========================================================");
    println!("Destination Folder Options:");
    println!("  [Enter] Use same directory ({})", same_dir.display());
    println!("  [b]     Browse for a destination folder");
    println!("  [Or paste a custom destination folder path directly]");

    let dest_input = prompt_line("\nDestination [Press Enter for same dir]: ")?;
    let dest_dir = match dest_input.as_str() {
        "" => same_dir,
        "b" | "B" | "browse" => interactive_navigator(
            &same_dir,
            BrowseMode::SelectDestinationFolder,
            "Select Destination Folder (Press Enter to Confirm)",
        )?,
        custom_path => clean_input_path(custom_path),
    };

    if is_archive {
        execute_decompress_flow(&selected_path, &dest_dir)
    } else {
        let algo_raw = prompt_line(
            "Compression mode [brotli (default, smallest) / zlib / lz4 / snappy]: ",
        )?;
        let algo = Algorithm::from_name(&algo_raw)?;
        execute_compress_flow(&selected_path, &dest_dir, algo, DEFAULT_CHUNK_SIZE)
    }
}

pub fn run_self_test() -> Result<()> {
    let sample_in = Path::new("sample_input.bin");
    let archive = Path::new("sample_archive.nolo");
    let sample_out = Path::new("sample_restored.bin");

    let payload: Vec<u8> = (0..32 * 1024 * 1024).map(|i| (i % 251) as u8).collect();
    fs::write(sample_in, &payload)?;

    let default_algo = Algorithm::default();
    println!(
        "Compressing 32 MB test file with {} across {} CPU threads...",
        default_algo.name(),
        rayon::current_num_threads()
    );
    compress_file_with_progress(sample_in, archive, default_algo, DEFAULT_CHUNK_SIZE, true)?;

    let comp_len = fs::metadata(archive)?.len();
    println!(
        "Compressed 32.00 MB -> {} ({:.2}% of original)",
        format_bytes(comp_len),
        (comp_len as f64 / payload.len() as f64) * 100.0
    );

    println!("Decompressing in parallel...");
    decompress_file_with_progress(archive, sample_out, true)?;

    assert_eq!(fs::read(sample_in)?, fs::read(sample_out)?);
    println!("Success! Checksums verified and files match byte-for-byte.");

    let _ = fs::remove_file(sample_in);
    let _ = fs::remove_file(archive);
    let _ = fs::remove_file(sample_out);
    Ok(())
}

pub fn print_usage() {
    println!(
        "zipnolo v0.2.0 — High-Ratio Lossless Block-Parallel Archiver for Linux, macOS & Windows ({} CPU threads)

USAGE:
  1. Interactive File/Folder Browser ({} to compress selected file/folder, default dest = same dir):
     zipnolo

  2. Compress a File OR Folder (defaults to Brotli-Max lossless & same directory):
     zipnolo compress <SOURCE_FILE_OR_FOLDER> [DEST_FOLDER] [--algo brotli|zlib|lz4|snappy] [--chunk-mb <MB>]

  3. Decompress a .nolo Archive (restores file or folder into same directory by default):
     zipnolo decompress <ARCHIVE.nolo> [DEST_FOLDER]

  4. Quick One-Command Compress / Decompress to Same Directory:
     zipnolo <FILE_OR_ARCHIVE>

  5. Run built-in 32 MB verification benchmark:
     zipnolo test",
        rayon::current_num_threads(),
        COMPRESS_KEY_LABEL
    );
}
