use crate::algorithm::Algorithm;
use crate::constants::{END_MAGIC, FOLDER_NAME_PREFIX, START_MAGIC};
use anyhow::{bail, Context, Result};
use std::fs::File;
use std::io::{BufWriter, Read, Seek, SeekFrom, Write};
use std::path::Path;

#[derive(Clone, Copy, Debug)]
pub struct BlockEntry {
    pub compressed_size: u32,
    pub checksum: u64,
}

pub struct CompressedBlock {
    pub index: usize,
    pub uncompressed_size: usize,
    pub data: Vec<u8>,
    pub checksum: u64,
}

pub struct ParsedFooter {
    pub orig_size: u64,
    pub chunk_size: usize,
    pub algo: Algorithm,
    pub is_folder: bool,
    pub stored_name: String,
    pub block_meta: Vec<(usize, usize, u64)>,
}

pub fn write_nolo_footer(
    writer: &mut BufWriter<File>,
    orig_size: u64,
    chunk_size: usize,
    algo: Algorithm,
    stored_name_bytes: &[u8],
    block_table: &[BlockEntry],
) -> Result<()> {
    let mut footer = Vec::new();
    footer.extend_from_slice(&orig_size.to_le_bytes());
    footer.extend_from_slice(&(chunk_size as u32).to_le_bytes());
    footer.push(algo as u8);
    footer.extend_from_slice(&(stored_name_bytes.len() as u16).to_le_bytes());
    footer.extend_from_slice(stored_name_bytes);
    footer.extend_from_slice(&(block_table.len() as u64).to_le_bytes());

    for entry in block_table {
        footer.extend_from_slice(&entry.compressed_size.to_le_bytes());
        footer.extend_from_slice(&entry.checksum.to_le_bytes());
    }

    let footer_len = footer.len() as u64;
    writer.write_all(&footer)?;
    writer.write_all(&footer_len.to_le_bytes())?;
    writer.write_all(END_MAGIC)?;
    writer.flush()?;
    Ok(())
}

pub fn read_archive_footer(archive_file: &mut File) -> Result<ParsedFooter> {
    let archive_len = archive_file.metadata()?.len();

    if archive_len < 16 {
        bail!("Invalid or corrupted NOLO archive (too small)");
    }

    // 1. Verify Start & End Magic and read Footer length
    archive_file.seek(SeekFrom::Start(0))?;
    let mut magic = [0u8; 4];
    archive_file.read_exact(&mut magic)?;
    if &magic != START_MAGIC {
        bail!("Invalid Start Magic: Not a NOLO archive");
    }

    archive_file.seek(SeekFrom::End(-12))?;
    let mut tail = [0u8; 12];
    archive_file.read_exact(&mut tail)?;
    if &tail[8..12] != END_MAGIC {
        bail!("Invalid End Magic: Archive may be truncated");
    }
    let footer_len = u64::from_le_bytes(tail[0..8].try_into()?);

    // 2. Read & Parse Footer with strict bounds validation
    let footer_start = archive_len
        .checked_sub(12 + footer_len)
        .context("Corrupted footer length")?;
    if footer_start < 4 {
        bail!("Corrupted footer offset overlaps Start Magic");
    }
    archive_file.seek(SeekFrom::Start(footer_start))?;

    let mut footer = vec![0u8; footer_len as usize];
    archive_file.read_exact(&mut footer)?;

    if footer.len() < 8 + 4 + 1 + 2 {
        bail!("Corrupted footer metadata (too short)");
    }

    let mut cursor = 0usize;
    let orig_size = u64::from_le_bytes(footer[cursor..cursor + 8].try_into()?);
    cursor += 8;
    let chunk_size = u32::from_le_bytes(footer[cursor..cursor + 4].try_into()?) as usize;
    cursor += 4;
    let algo = Algorithm::try_from(footer[cursor])?;
    cursor += 1;
    let name_len = u16::from_le_bytes(footer[cursor..cursor + 2].try_into()?) as usize;
    cursor += 2;
    if cursor + name_len + 8 > footer.len() {
        bail!("Corrupted footer filename or block count");
    }
    let raw_name = std::str::from_utf8(&footer[cursor..cursor + name_len])
        .context("Invalid UTF-8 filename in archive footer")?;

    let (is_folder, clean_name) = if let Some(folder_raw) = raw_name.strip_prefix(FOLDER_NAME_PREFIX)
    {
        (true, folder_raw)
    } else {
        (false, raw_name)
    };

    // Sanitize stored name to prevent path traversal attacks
    let stored_name = Path::new(clean_name)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("restored_item")
        .to_string();
    cursor += name_len;
    let num_blocks = u64::from_le_bytes(footer[cursor..cursor + 8].try_into()?) as usize;
    cursor += 8;

    if cursor + num_blocks * 12 > footer.len() {
        bail!("Corrupted footer block table");
    }

    let mut block_meta = Vec::with_capacity(num_blocks);
    let mut current_offset = 4usize; // Starts right after START_MAGIC
    for _ in 0..num_blocks {
        let comp_size = u32::from_le_bytes(footer[cursor..cursor + 4].try_into()?) as usize;
        let checksum = u64::from_le_bytes(footer[cursor + 4..cursor + 12].try_into()?);
        cursor += 12;
        block_meta.push((current_offset, comp_size, checksum));
        current_offset += comp_size;
    }

    Ok(ParsedFooter {
        orig_size,
        chunk_size,
        algo,
        is_folder,
        stored_name,
        block_meta,
    })
}
