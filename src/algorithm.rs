use anyhow::{bail, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Algorithm {
    Lz4 = 1,
    Snappy = 2,
}

impl Default for Algorithm {
    fn default() -> Self {
        Algorithm::Lz4
    }
}

impl TryFrom<u8> for Algorithm {
    type Error = anyhow::Error;
    fn try_from(v: u8) -> Result<Self> {
        match v {
            1 => Ok(Algorithm::Lz4),
            2 => Ok(Algorithm::Snappy),
            _ => bail!("Unsupported compression algorithm ID: {}", v),
        }
    }
}

impl Algorithm {
    pub fn from_name(name: &str) -> Result<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "" | "lz4" | "fast" | "l" | "1" => Ok(Algorithm::Lz4),
            "snappy" | "snap" | "s" | "2" => Ok(Algorithm::Snappy),
            other => bail!("Unknown algorithm '{}'. Valid options: lz4, snappy", other),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Algorithm::Lz4 => "LZ4 (Fast)",
            Algorithm::Snappy => "Snappy (Fast)",
        }
    }
}
