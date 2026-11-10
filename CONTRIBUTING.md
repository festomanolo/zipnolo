# Contributing to `zipnolo`

Thank you for your interest in contributing to **`zipnolo`**, created by **[festomanolo](https://festomanolo.com)**!

## Development Workflow

1. **Clone the repository**:
   ```bash
   git clone https://github.com/festomanolo/zipnolo.git
   cd zipnolo
   ```
2. **Build & Run Tests**:
   ```bash
   cargo test
   cargo run --release -- test
   ```
3. **Architecture Invariants**:
   - **64-bit Only**: `src/lib.rs` enforces `#[cfg(not(target_pointer_width = "64"))]` for virtual memory mapping.
   - **Zero-Byte File Safety**: Never call `Mmap::map` on a `0`-byte file (`file_size == 0`).
   - **Explicit Mmap Drop Ordering**: Always `drop(mmap)` / `drop(out_mmap)` before closing or modifying file descriptors.
   - **Path Traversal Protection**: All filenames and folder relative paths extracted from `.nolo` archives must strip `..`, `.`, and leading root slashes.
   - **Read-Only Overwrite Safety**: Always call `prepare_overwrite_path` before recreating target files so read-only files (e.g., `.git/objects/*` with mode `0o444`) are cleanly replaced.
