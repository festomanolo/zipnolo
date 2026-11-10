# CLAUDE.md — Complete Architectural & Engineering Guide for `zipnolo`

**Project**: `zipnolo` (v0.2.0)  
**Author / Creator**: **festomanolo** ([https://festomanolo.com](https://festomanolo.com))  
**Repository**: [https://github.com/festomanolo/zipnolo](https://github.com/festomanolo/zipnolo)  
**PyPI Package**: `pip install zipnolo`  
**Language**: 100% Pure Safe Rust (Edition 2021, 64-bit target architectures)

---

## 1. Executive Summary

`zipnolo` is a high-ratio, lossless, block-parallel file and folder archiver designed for **Linux**, **macOS**, and **Windows**. It solves the three primary bottlenecks of traditional archivers (`tar.gz`, `zip`, `xz`):

1. **Bounded Streaming Pipelining (`crossbeam-channel` + `rayon`)**:
   - Compressing a 50 GB or 1 TB file or folder never buffers the archive in RAM. A bounded channel (`max(4, 2 * CPU_CORES)`) enforces natural backpressure between the Rayon worker pool and the sequential disk writer, capping memory usage to a few megabytes.
2. **Single-Pass Footer-Indexed Format (`NOLO` v1)**:
   - By placing the Block Index Table and Metadata at the **end of the file (Footer)**—similar to Apache Parquet and `.zip`—compressed blocks are flushed to disk the millisecond they finish in order, with zero backward seeking during writes and instant `SeekFrom::End(-12)` index lookup during reads.
3. **Parallel Zero-Copy Decompression (`memmap2::MmapMut`)**:
   - For single-file archives, `zipnolo` pre-allocates the exact uncompressed file size on disk via `File::set_len()`, maps the output file with `MmapMut`, splits it into disjoint mutable slices (`par_chunks_mut`), and decompresses all blocks across all CPU cores simultaneously directly into OS virtual memory with zero intermediate heap allocations.
4. **Maximum Lossless Compression + Raw Block Bypass**:
   - Defaults to **Brotli-Max (`Algorithm::Brotli`)** (4 MB sliding window `lgwin=22`, quality 9–11) across all CPU cores in parallel, achieving ~80% size reduction on source code and structured data, while also supporting **Zlib-9 (`Algorithm::Zlib`)**, **LZ4 (`Algorithm::Lz4`)**, and **Snappy (`Algorithm::Snappy`)**.
   - If any block is incompressible (random, encrypted, or pre-compressed media where `compressed.len() >= chunk.len()`), `zipnolo` automatically stores the raw uncompressed chunk (`compressed_size == uncompressed_size`), mathematically guaranteeing **zero block expansion (`<= 100.0%`)**.
5. **Per-Block 64-Bit `XXH3` Integrity Verification**:
   - Every uncompressed block is hashed with `xxh3_64` (>30 GB/s) inside each worker thread and verified on decompression to detect bit rot or storage corruption immediately.

---

## 2. Codebase Module Map

```text
/Volumes/MacX/projects/compress
├── Cargo.toml                 # Rust crate manifest (zipnolo v0.2.0, LTO release profile)
├── pyproject.toml             # PyPI / Maturin packaging manifest (pip install zipnolo)
├── install.sh                 # Cross-platform Linux & macOS installer script
├── Makefile                   # Standard build, release, test, install targets
├── README.md                  # User-facing documentation & quickstart
├── CLAUDE.md                  # Comprehensive engineering & architecture guide (this file)
├── docs/
│   └── ARCHITECTURE.md        # NOLO v1 & NLDR v1 binary format specification
├── src/
│   ├── lib.rs                 # Library root, 64-bit compile_error! guard, public exports
│   ├── main.rs                # CLI argument parser and entrypoint for `zipnolo`
│   ├── constants.rs           # Magic headers (NOLO, OLON, NLDR), chunk size (4 MB), OS key labels
│   ├── algorithm.rs           # Algorithm enum (Lz4=1, Snappy=2, Brotli=3, Zlib=4) & parser
│   ├── codec.rs               # Block compression/decompression + incompressible raw-bypass
│   ├── format.rs              # NOLO v1 footer serializer, parser, bounds checks & path sanitization
│   ├── progress.rs            # Lock-free AtomicU64 terminal progress bar (25 FPS throttled)
│   ├── archiver.rs            # Single-file Mmap/MmapMut parallel compressor & decompressor
│   ├── folder.rs              # Recursive folder walker, NLDR container streamer & parallel extractor
│   ├── paths.rs               # Same-directory default destination resolver & path sanitizer
│   ├── navigator.rs           # Interactive crossterm TUI file/folder browser (Cmd+Enter / Ctrl+Enter)
│   └── cli.rs                 # Interactive workflows, compress/decompress runners, 32 MB self-test
└── tests/
    └── integration_tests.rs   # End-to-end test suite (0-byte, unaligned, corruption, folders, Brotli)
```

---

## 3. Binary Format Specifications

### 3.1 `NOLO` v1 Archive Layout

| Section | Size | Type / Endianness | Description |
| :--- | :--- | :--- | :--- |
| **Start Magic** | 4 Bytes | `[u8; 4]` (`b"NOLO"`) | Identifies file as a NOLO archive |
| **Compressed Blocks** | Variable | Raw Bytes | `[Block 0][Block 1]...[Block N-1]` written sequentially |
| **Footer Metadata** | Variable | Little-Endian | See Footer Metadata layout below |
| **Footer Size** | 8 Bytes | `u64` (LE) | Total byte length of the Footer Metadata section |
| **End Magic** | 4 Bytes | `[u8; 4]` (`b"OLON"`) | Verifies archive is not truncated; enables `SeekFrom::End(-12)` |

#### Footer Metadata Fields (Little-Endian):
1. `orig_size`: `u64` (8 bytes) — Uncompressed byte length of the file (or `NLDR` folder stream).
2. `chunk_size`: `u32` (4 bytes) — Uncompressed block chunk size (default `4,194,304` bytes / 4 MB).
3. `algo_id`: `u8` (1 byte) — `1 = Lz4`, `2 = Snappy`, `3 = Brotli`, `4 = Zlib`.
4. `name_len`: `u16` (2 bytes) — Byte length of the UTF-8 stored filename.
5. `stored_name`: `[u8; name_len]` — UTF-8 filename (e.g. `"video.mp4"`) or folder tag (`"NOLO_DIR:my_folder"`).
6. `num_blocks`: `u64` (8 bytes) — Number of blocks in the archive.
7. `block_table`: `[BlockEntry; num_blocks]` (12 bytes per block):
   - `compressed_size`: `u32` (4 bytes)
   - `xxh3_checksum`: `u64` (8 bytes)

### 3.2 `NLDR` v1 Directory Container Stream (Inside Folder Archives)

When compressing a directory, `stored_name` in the `NOLO` footer is prefixed with `"NOLO_DIR:<folder_name>"` and the uncompressed stream consists of:
- `magic`: `b"NLDR"` (4 bytes)
- `item_count`: `u64` (8 bytes, LE)
- Repeated `item_count` times:
  - `kind`: `u8` (`0` = directory, `1` = regular file, `2` = executable file with Unix `0o755`)
  - `rel_path_len`: `u32` (4 bytes, LE)
  - `rel_path`: `[u8; rel_path_len]` (UTF-8 forward-slash normalized relative path)
  - `file_size`: `u64` (8 bytes, LE; `0` for directories)
  - `file_data`: `[u8; file_size]`

---

## 4. Cross-Platform Compatibility (Linux, macOS, Windows)

1. **Zero-Byte Files**: Calling `mmap` on a 0-byte file triggers `EINVAL` on Linux/macOS and `ERROR_FILE_INVALID` on Windows. `zipnolo` checks `file_size > 0` before mapping and handles 0-byte files as metadata-only archives.
2. **64-Bit Virtual Address Space**: Enforced at compile time via `#[cfg(not(target_pointer_width = "64"))] compile_error!(...)` in `src/lib.rs`.
3. **Windows File Locking**: `Mmap` and `MmapMut` handles are explicitly dropped (`drop(out_mmap); drop(in_mmap);`) before closing files.
4. **Linux Special Files & Permissions**: Directory traversal uses `fs::symlink_metadata` so broken symlinks, sockets, and FIFOs never cause errors on Linux, and Unix executable bits (`0o111`) are preserved across compression and extraction.
5. **Terminal Keyboard Protocol (`Cmd+Enter` / `Ctrl+Enter`)**: `RawModeGuard` enables `crossterm::event::PushKeyboardEnhancementFlags` (Kitty protocol) so terminals report `Cmd+Enter` (`KeyModifiers::SUPER`), `Ctrl+Enter`, `Alt+Enter`, or `c`/`Space` to compress any highlighted file or folder while `Enter`/`→` opens folders to navigate inside them.

---

## 5. Development, Testing & Packaging Commands

```bash
# Run full integration test suite (6 tests)
cargo test

# Run 32 MB parallel self-test benchmark
cargo run --release -- test

# Install locally on Linux or macOS
./install.sh

# Build PyPI wheels and source distribution via Maturin
maturin build --release
maturin sdist
```
