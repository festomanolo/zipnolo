use anyhow::{bail, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Algorithm {
    Lz4 = 1,
    Snappy = 2,
    Brotli = 3,
    Zlib = 4,
}

impl Default for Algorithm {
    fn default() -> Self {
        Algorithm::Brotli
    }
}

impl TryFrom<u8> for Algorithm {
    type Error = anyhow::Error;
    fn try_from(v: u8) -> Result<Self> {
        match v {
            1 => Ok(Algorithm::Lz4),
            2 => Ok(Algorithm::Snappy),
            3 => Ok(Algorithm::Brotli),
            4 => Ok(Algorithm::Zlib),
            _ => bail!("Unsupported compression algorithm ID: {}", v),
        }
    }
}

impl Algorithm {
    pub fn from_name(name: &str) -> Result<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "" | "brotli" | "br" | "max" | "ultra" | "best" | "b" | "3" => Ok(Algorithm::Brotli),
            "zlib" | "deflate" | "gz" | "z" | "4" => Ok(Algorithm::Zlib),
            "lz4" | "fast" | "l" | "1" => Ok(Algorithm::Lz4),
            "snappy" | "snap" | "s" | "2" => Ok(Algorithm::Snappy),
            other => bail!(
                "Unknown algorithm '{}'. Valid options: brotli (default, max compression), zlib, lz4, snappy",
                other
            ),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Algorithm::Brotli => "Brotli-Max (Lossless Ultra)",
            Algorithm::Zlib => "Zlib-9 (Lossless Best)",
            Algorithm::Lz4 => "LZ4 (Fast)",
            Algorithm::Snappy => "Snappy (Fast)",
        }
    }
}
