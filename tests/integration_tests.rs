use anyhow::Result;
use std::fs;
use zipnolo::{
    compress_file, decompress_file, Algorithm, DEFAULT_CHUNK_SIZE,
};

#[test]
fn test_zero_byte_file() -> Result<()> {
    let dir = std::env::temp_dir();
    let input = dir.join("nolo_zero_in.bin");
    let archive = dir.join("nolo_zero.nolo");
    let output = dir.join("nolo_zero_out.bin");

    fs::write(&input, b"")?;
    compress_file(&input, &archive, Algorithm::Brotli, DEFAULT_CHUNK_SIZE)?;
    decompress_file(&archive, &output)?;

    assert_eq!(fs::read(&output)?.len(), 0);

    let _ = fs::remove_file(&input);
    let _ = fs::remove_file(&archive);
    let _ = fs::remove_file(&output);
    Ok(())
}

#[test]
fn test_all_algorithms_roundtrip_unaligned() -> Result<()> {
    let dir = std::env::temp_dir();
    let input = dir.join("nolo_unaligned_in.bin");
    let payload: Vec<u8> = (0..(1536 * 1024 + 137)).map(|i| (i % 251) as u8).collect();
    fs::write(&input, &payload)?;

    let chunk_size = 256 * 1024;
    for algo in [
        Algorithm::Brotli,
        Algorithm::Zlib,
        Algorithm::Lz4,
        Algorithm::Snappy,
    ] {
        let archive = dir.join(format!("nolo_unaligned_{:?}.nolo", algo));
        let output = dir.join(format!("nolo_unaligned_{:?}_out.bin", algo));

        compress_file(&input, &archive, algo, chunk_size)?;
        decompress_file(&archive, &output)?;
        assert_eq!(fs::read(&output)?, payload);

        let _ = fs::remove_file(&archive);
        let _ = fs::remove_file(&output);
    }

    let _ = fs::remove_file(&input);
    Ok(())
}

#[test]
fn test_checksum_corruption_detection() -> Result<()> {
    let dir = std::env::temp_dir();
    let input = dir.join("nolo_corrupt_in.bin");
    let archive = dir.join("nolo_corrupt.nolo");
    let output = dir.join("nolo_corrupt_out.bin");

    let payload: Vec<u8> = vec![42u8; 128 * 1024];
    fs::write(&input, &payload)?;
    compress_file(&input, &archive, Algorithm::Brotli, 64 * 1024)?;

    let mut archive_bytes = fs::read(&archive)?;
    let checksum_byte_idx = archive_bytes.len() - 12 - 1;
    archive_bytes[checksum_byte_idx] ^= 0xFF;
    fs::write(&archive, &archive_bytes)?;

    let res = decompress_file(&archive, &output);
    assert!(res.is_err());
    let err_msg = format!("{:#}", res.unwrap_err());
    assert!(
        err_msg.contains("failed XXH3 checksum verification"),
        "Unexpected error: {}",
        err_msg
    );

    let _ = fs::remove_file(&input);
    let _ = fs::remove_file(&archive);
    let _ = fs::remove_file(&output);
    Ok(())
}
