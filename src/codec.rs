use crate::algorithm::Algorithm;
use anyhow::{bail, Context, Result};
use flate2::{read::ZlibDecoder, write::ZlibEncoder, Compression as FlateCompression};
use std::io::{Read, Write};

/// Compresses a single block using pure-Rust Brotli, Zlib, LZ4, or Snappy.
/// Guarantees zero block expansion: if the compressed output is >= the original chunk size
/// (e.g. random or pre-compressed data), stores the raw uncompressed block directly.
pub fn compress_chunk(chunk: &[u8], algo: Algorithm) -> Result<Vec<u8>> {
    if chunk.is_empty() {
        return Ok(Vec::new());
    }

    let compressed = match algo {
        Algorithm::Brotli => {
            let mut out = Vec::with_capacity(chunk.len() / 2 + 64);
            // Quality 11 for chunks <= 512 KB, Quality 9 for multi-MB chunks for optimal density/throughput
            let quality: u32 = if chunk.len() <= 512 * 1024 { 11 } else { 9 };
            let lgwin: u32 = 22; // 4 MB sliding window
            {
                let mut writer = brotli::CompressorWriter::new(&mut out, 64 * 1024, quality, lgwin);
                writer.write_all(chunk)?;
                writer.flush()?;
            }
            out
        }
        Algorithm::Zlib => {
            let mut encoder = ZlibEncoder::new(
                Vec::with_capacity(chunk.len() / 2 + 64),
                FlateCompression::best(),
            );
            encoder.write_all(chunk)?;
            encoder.finish()?
        }
        Algorithm::Lz4 => lz4_flex::compress(chunk),
        Algorithm::Snappy => {
            let mut encoder = snap::raw::Encoder::new();
            encoder.compress_vec(chunk)?
        }
    };

    // Raw block bypass: never allow an incompressible block to expand
    if compressed.len() >= chunk.len() {
        Ok(chunk.to_vec())
    } else {
        Ok(compressed)
    }
}

/// Decompresses a single block directly into the pre-allocated output slice.
/// Automatically detects raw-bypassed incompressible blocks when `compressed.len() == out_slice.len()`.
pub fn decompress_chunk_into(compressed: &[u8], out_slice: &mut [u8], algo: Algorithm) -> Result<()> {
    if compressed.len() == out_slice.len() {
        out_slice.copy_from_slice(compressed);
        return Ok(());
    }

    match algo {
        Algorithm::Brotli => {
            let mut reader = brotli::Decompressor::new(compressed, 64 * 1024);
            reader
                .read_exact(out_slice)
                .context("Brotli decompression error")?;
        }
        Algorithm::Zlib => {
            let mut decoder = ZlibDecoder::new(compressed);
            decoder
                .read_exact(out_slice)
                .context("Zlib decompression error")?;
        }
        Algorithm::Lz4 => {
            let bytes = lz4_flex::decompress_into(compressed, out_slice)
                .map_err(|e| anyhow::anyhow!("LZ4 decompression error: {}", e))?;
            if bytes != out_slice.len() {
                bail!("Decompressed size mismatch");
            }
        }
        Algorithm::Snappy => {
            let mut decoder = snap::raw::Decoder::new();
            let bytes = decoder.decompress(compressed, out_slice)?;
            if bytes != out_slice.len() {
                bail!("Decompressed size mismatch");
            }
        }
    }
    Ok(())
}
