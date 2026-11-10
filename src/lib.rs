#[cfg(not(target_pointer_width = "64"))]
compile_error!("zipnolo requires a 64-bit target architecture (x86_64 or aarch64) for large virtual address space memory mapping.");

pub mod algorithm;
pub mod archiver;
pub mod cli;
pub mod codec;
pub mod constants;
pub mod folder;
pub mod format;
pub mod navigator;
pub mod paths;
pub mod progress;

pub use algorithm::Algorithm;
pub use archiver::{
    compress_file, compress_file_with_progress, decompress_file, decompress_file_with_progress,
};
pub use codec::{compress_chunk, decompress_chunk_into};
pub use constants::DEFAULT_CHUNK_SIZE;
pub use folder::compress_folder_with_progress;
pub use paths::{
    clean_input_path, default_same_dir, is_nolo_archive, resolve_compress_destination,
    resolve_decompress_destination,
};
pub use progress::{format_bytes, ProgressBar};
