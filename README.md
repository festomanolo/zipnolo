# zipnolo

**`zipnolo`** is a blazingly fast, cross-platform (Linux, macOS, Windows), block-parallel lossless file and folder archiver written in 100% pure Safe Rust.

**Created by [festomanolo](https://festomanolo.com) ([festomanolo.com](https://festomanolo.com))**

---

## Key Features

- **Maximum Lossless Compression Ratio**: Defaults to parallel **Brotli-Max** (`lgwin=22`, quality 9–11) across all CPU cores, achieving ~80% space savings on source code and structured data while also supporting **Zlib-9**, **LZ4**, and **Snappy**.
- **Incompressible Block Raw-Bypass**: Automatically detects random or already-compressed blocks and stores them raw so blocks never expand beyond 100%.
- **Files & Entire Folders**: Compress individual files or whole directory trees (preserving nested subdirectories, empty folders, and Unix executable permissions on Linux/macOS).
- **Interactive Terminal File/Folder Navigator**: Run `zipnolo` anywhere in your terminal to browse files and directories with arrow keys, press **`Cmd+Enter`** (macOS) or **`Ctrl+Enter` / `Alt+Enter` / `c`** (Linux/Windows) to compress any highlighted file or folder, or press **`p`** to paste a path.
- **Same-Directory Default Destination**: Automatically defaults to saving archives and extracting files in the same directory as the source item unless another destination folder is chosen.
- **Bounded RAM + Per-Block `XXH3` Checksums**: Constant memory usage via bounded channels (`crossbeam-channel` + `rayon`) and 64-bit `XXH3` integrity verification on every block.

---

## Global Installation (`pip`, `cargo`, or `install.sh`)

### 1. Install via PyPI (`pip`) — Works Worldwide on Linux, macOS & Windows
```bash
pip install zipnolo
```

### 2. Install via Cargo (Rust)
```bash
cargo install zipnolo
# or from source:
git clone https://github.com/festomanolo/zipnolo.git
cd zipnolo
./install.sh
```

---

## Usage

### 1. Interactive Terminal Browser
Run `zipnolo` from any directory:
```bash
zipnolo
```
- **`↑` / `↓` (or `j` / `k`)**: Navigate folders and files.
- **`Enter` / `→` on a folder**: Open folder to navigate inside.
- **`Cmd+Enter` (macOS) / `Ctrl+Enter` or `Alt+Enter` or `c` (Linux)**: Compress the highlighted file or folder (or decompress a `.nolo` archive).
- **`←` / `Backspace`**: Go to parent directory (`..`).
- **`p`**: Paste a directory or file path directly.

### 2. Direct CLI Commands
```bash
# Compress a file or folder (defaults to Brotli-Max lossless & same directory)
zipnolo compress /path/to/my_folder
zipnolo compress /path/to/file.bin /custom/destination/folder --algo brotli

# Decompress a .nolo archive (automatically restores file or folder tree)
zipnolo decompress /path/to/my_folder.nolo

# Run built-in 32 MB parallel verification benchmark
zipnolo test
```

---

## Author & Credits

Created and maintained by **[festomanolo](https://festomanolo.com)**
- Website: **[https://festomanolo.com](https://festomanolo.com)**
- GitHub: **[https://github.com/festomanolo/zipnolo](https://github.com/festomanolo/zipnolo)**

## License
MIT © [festomanolo](https://festomanolo.com)
