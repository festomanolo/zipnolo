use crate::algorithm::Algorithm;
use anyhow::{bail, Result};

/// Compresses a single block using pure-Rust LZ4 or Snappy.
pub fn compress_chunk(chunk: &[u8], algo: Algorithm) -> Result<Vec<u8>> {
    if chunk.is_empty() {
        return Ok(Vec::new());
    }
    match algo {
        Algorithm::Lz4 => Ok(lz4_flex::compress(chunk)),
        Algorithm::Snappy => {
            let mut encoder = snap::raw::Encoder::new();
            Ok(encoder.compress_vec(chunk)?)
        }
    }
}

/// Decompresses a single block directly into the pre-allocated output slice.
pub fn decompress_chunk_into(compressed: &[u8], out_slice: &mut [u8], algo: Algorithm) -> Result<()> {
    match algo {
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
