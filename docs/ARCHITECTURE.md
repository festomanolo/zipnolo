# ZIPNOLO (`NOLO` v1 / `NLDR` v1) Architecture Specification

## 1. Core Architectural Pillars

1. **Footer-Indexed Archive Layout (`NOLO` v1)**:
   - Placing the Block Index Table and Metadata at the **end of the archive** (like `.zip` and Apache Parquet) allows single-pass sequential writes without seeking back or holding gigabytes of compressed blocks in RAM.
2. **Bounded Streaming Pipeline (`crossbeam-channel` + `rayon`)**:
   - In-flight blocks are bounded to `max(4, 2 * CPU_CORES)`. If disk writes are slower than multi-core compression, natural backpressure caps memory usage to a few megabytes regardless of whether the input is 100 MB or 100 GB.
3. **Zero-Copy Parallel Decompression (`memmap2::MmapMut`)**:
   - For single-file archives, `zipnolo` pre-allocates the output file on disk (`File::set_len`) and splits a mutable memory map (`MmapMut::par_chunks_mut`) across all CPU cores simultaneously without intermediate heap buffers.
4. **High-Ratio Lossless Compression + Raw Block Bypass**:
   - Supports pure-Rust **Brotli-Max** (default, sliding window 4 MB, quality 9–11), **Zlib-9**, **LZ4**, and **Snappy**.
   - If any block is incompressible (`compressed.len() >= chunk.len()`), it is stored raw (`compressed_size == uncompressed_size`), guaranteeing zero expansion on random or pre-compressed data.
5. **Per-Block 64-Bit Integrity Verification (`XXH3`)**:
   - Every block is hashed with `xxh3_64` prior to compression and verified immediately upon decompression.

## 2. Binary Layout (`NOLO` v1)

| Section | Size | Type / Endianness | Description |
| :--- | :--- | :--- | :--- |
| **Start Magic** | 4 Bytes | `b"NOLO"` | Archive magic header |
| **Compressed Blocks** | Variable | Raw Bytes | `[Block 0][Block 1]...[Block N-1]` |
| **Footer Metadata** | Variable | Little-Endian | `orig_size: u64`, `chunk_size: u32`, `algo_id: u8`, `name_len: u16`, `name: [u8]`, `num_blocks: u64`, `entries: [(compressed_size: u32, xxh3_checksum: u64); num_blocks]` |
| **Footer Size** | 8 Bytes | `u64` (LE) | Byte length of Footer Metadata |
| **End Magic** | 4 Bytes | `b"OLON"` | Verifies archive integrity and enables `SeekFrom::End(-12)` lookup |
