# Changelog

All notable changes to **`zipnolo`** (created by **[festomanolo](https://festomanolo.com)**) are documented in this file.

## [0.2.1] - 2026-11-10

### Fixed
- **Read-Only File Overwrite (`os error 13`)**: Fixed `Permission denied (os error 13)` when extracting a `.nolo` folder archive over an existing directory containing read-only files such as `.git/objects/pack/*` or `.git/objects/??/*` (`0o444`). Added `ensure_dir_writable` and `prepare_overwrite_path` before file extraction.

### Added
- **`zipnolo info <archive.nolo>`**: Inspect `.nolo` archive footer metadata (original size, compressed size, ratio, algorithm, block count, chunk size, and container type) in O(1) time without extracting.
- **Regression Test**: Added `test_folder_decompress_overwrites_readonly_git_objects` in `tests/integration_tests.rs`.

## [0.2.0] - 2026-11-10

### Added
- **Brotli-Max (`Brotli`, ID `3`) & Zlib-9 (`Zlib`, ID `4`)**: Pure-Rust high-ratio lossless compression algorithms alongside `Lz4` and `Snappy`.
- **Incompressible Block Raw-Bypass**: Per-block `0x00` (raw) / `0x01` (compressed) framing header guarantees zero size expansion on incompressible data.
- **Full Directory Archiving (`NLDR` stream)**: Recursively compress and decompress entire folders with relative path sanitization and Unix executable permission (`0o755`) preservation.
- **Interactive Terminal Navigator**: Full-screen `crossterm` TUI with `Cmd+Enter` (macOS) / `Ctrl+Enter` / `Alt+Enter` / `c` (Linux & Windows) keybindings and default same-directory destination.
- **PyPI Distribution**: Published native binary wheels and sdist via `maturin` (`pip install zipnolo`).

## [0.1.0] - 2026-11-10

### Added
- Initial `NOLO` v1 footer-indexed block-parallel archiver with bounded `crossbeam-channel` + `rayon` pipeline, zero-copy `MmapMut` parallel decompression, and 64-bit `XXH3` block checksums.
