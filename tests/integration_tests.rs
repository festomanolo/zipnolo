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

use zipnolo::{
    compress_chunk, compress_file_with_progress, compress_folder_with_progress,
    decompress_chunk_into, decompress_file_with_progress, default_same_dir,
    resolve_compress_destination, resolve_decompress_destination,
};

#[test]
fn test_brotli_significantly_smaller_than_lz4_and_incompressible_raw_bypass() -> Result<()> {
    let sample_code = include_bytes!("../src/archiver.rs");
    let brotli_comp = compress_chunk(sample_code, Algorithm::Brotli)?;
    let lz4_comp = compress_chunk(sample_code, Algorithm::Lz4)?;
    assert!(
        brotli_comp.len() < lz4_comp.len(),
        "Expected Brotli ({}) to be smaller than LZ4 ({})",
        brotli_comp.len(),
        lz4_comp.len()
    );
    let mut restored = vec![0u8; sample_code.len()];
    decompress_chunk_into(&brotli_comp, &mut restored, Algorithm::Brotli)?;
    assert_eq!(restored, sample_code);

    let mut rng_state: u64 = 0x9E37_79B9_7F4A_7C15;
    let random_data: Vec<u8> = (0..65536)
        .map(|_| {
            rng_state ^= rng_state << 13;
            rng_state ^= rng_state >> 7;
            rng_state ^= rng_state << 17;
            (rng_state & 0xFF) as u8
        })
        .collect();
    let raw_bypassed = compress_chunk(&random_data, Algorithm::Brotli)?;
    assert!(raw_bypassed.len() <= random_data.len());
    let mut restored_rand = vec![0u8; random_data.len()];
    decompress_chunk_into(&raw_bypassed, &mut restored_rand, Algorithm::Brotli)?;
    assert_eq!(restored_rand, random_data);

    Ok(())
}

#[test]
fn test_destination_folder_and_default_same_dir() -> Result<()> {
    let base_dir = std::env::temp_dir().join("zipnolo_same_dir_test");
    let src_dir = base_dir.join("src_dir");
    let restored_dir = base_dir.join("restored_out_dir");

    let _ = fs::remove_dir_all(&base_dir);
    fs::create_dir_all(&src_dir)?;

    let src_file = src_dir.join("demo_payload.dat");
    let payload: Vec<u8> = (0..512 * 1024).map(|i| (i % 199) as u8).collect();
    fs::write(&src_file, &payload)?;

    let same_dir = default_same_dir(&src_file);
    assert_eq!(same_dir, fs::canonicalize(&src_dir)?);

    let archive_path = resolve_compress_destination(&src_file, &same_dir)?;
    assert_eq!(
        archive_path,
        fs::canonicalize(&src_dir)?.join("demo_payload.dat.nolo")
    );
    compress_file_with_progress(&src_file, &archive_path, Algorithm::Brotli, 128 * 1024, false)?;

    let restored_path = resolve_decompress_destination(&archive_path, &restored_dir)?;
    assert_eq!(restored_path, restored_dir.join("demo_payload.dat"));
    decompress_file_with_progress(&archive_path, &restored_path, false)?;

    assert_eq!(fs::read(&restored_path)?, payload);

    let _ = fs::remove_dir_all(&base_dir);
    Ok(())
}

#[test]
fn test_folder_compress_and_decompress_roundtrip() -> Result<()> {
    let base_dir = std::env::temp_dir().join("zipnolo_folder_roundtrip_test");
    let src_folder = base_dir.join("my_project");
    let sub_a = src_folder.join("src").join("nested");
    let empty_dir = src_folder.join("empty_subdir");
    let extract_dir = base_dir.join("extracted");

    let _ = fs::remove_dir_all(&base_dir);
    fs::create_dir_all(&sub_a)?;
    fs::create_dir_all(&empty_dir)?;

    let file1_bytes: Vec<u8> = (0..300 * 1024).map(|i| (i % 251) as u8).collect();
    let file2_bytes = b"Hello from nested directory inside zipnolo folder archive!".to_vec();
    fs::write(src_folder.join("root_data.bin"), &file1_bytes)?;
    fs::write(sub_a.join("code.rs"), &file2_bytes)?;
    fs::write(src_folder.join("empty.txt"), b"")?;

    let same_dir = default_same_dir(&src_folder);
    let archive_path = resolve_compress_destination(&src_folder, &same_dir)?;
    assert_eq!(
        archive_path,
        fs::canonicalize(&base_dir)?.join("my_project.nolo")
    );

    compress_folder_with_progress(
        &src_folder,
        &archive_path,
        Algorithm::Brotli,
        128 * 1024,
        false,
    )?;

    let restored_folder = resolve_decompress_destination(&archive_path, &extract_dir)?;
    assert_eq!(restored_folder, extract_dir.join("my_project"));
    decompress_file_with_progress(&archive_path, &restored_folder, false)?;

    assert_eq!(fs::read(restored_folder.join("root_data.bin"))?, file1_bytes);
    assert_eq!(
        fs::read(restored_folder.join("src").join("nested").join("code.rs"))?,
        file2_bytes
    );
    assert_eq!(fs::read(restored_folder.join("empty.txt"))?.len(), 0);
    assert!(restored_folder.join("empty_subdir").is_dir());

    let _ = fs::remove_dir_all(&base_dir);
    Ok(())
}

#[test]
fn test_folder_decompress_overwrites_readonly_git_objects() -> Result<()> {
    let base_dir = std::env::temp_dir().join("zipnolo_readonly_git_test");
    let src_folder = base_dir.join("cheza_repo");
    let git_obj_dir = src_folder.join(".git").join("objects").join("59");

    let _ = fs::remove_dir_all(&base_dir);
    fs::create_dir_all(&git_obj_dir)?;

    let readonly_file = git_obj_dir.join("ee2b6e965e1ccab1a9c17eca1c3e0b30579fff");
    let obj_payload = b"git pack/object immutable content 0o444".to_vec();
    fs::write(&readonly_file, &obj_payload)?;

    // Mark file read-only (0o444 on Unix, readonly=true on Windows) just like Git objects
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&readonly_file, fs::Permissions::from_mode(0o444))?;
    }
    #[cfg(not(unix))]
    {
        let mut perms = fs::metadata(&readonly_file)?.permissions();
        perms.set_readonly(true);
        fs::set_permissions(&readonly_file, perms)?;
    }

    let same_dir = default_same_dir(&src_folder);
    let archive_path = resolve_compress_destination(&src_folder, &same_dir)?;
    compress_folder_with_progress(&src_folder, &archive_path, Algorithm::Brotli, 64 * 1024, false)?;

    // Decompress directly into the SAME directory where `cheza_repo` and its 0o444 `.git/objects` already exist
    let restored_folder = resolve_decompress_destination(&archive_path, &same_dir)?;
    decompress_file_with_progress(&archive_path, &restored_folder, false)?;

    assert_eq!(fs::read(&readonly_file)?, obj_payload);

    let _ = fs::remove_dir_all(&base_dir);
    Ok(())
}

#[test]
fn test_archive_info_flow_on_file_and_folder() -> Result<()> {
    let base_dir = std::env::temp_dir().join("zipnolo_info_flow_test");
    let src_folder = base_dir.join("sample_folder");
    let _ = fs::remove_dir_all(&base_dir);
    fs::create_dir_all(&src_folder)?;

    let file_path = src_folder.join("hello.txt");
    fs::write(&file_path, b"zipnolo metadata inspection payload")?;

    let file_archive = base_dir.join("hello.txt.nolo");
    compress_file(&file_path, &file_archive, Algorithm::Brotli, 64 * 1024)?;
    zipnolo::cli::execute_info_flow(&file_archive)?;

    let folder_archive = base_dir.join("sample_folder.nolo");
    compress_folder_with_progress(&src_folder, &folder_archive, Algorithm::Brotli, 64 * 1024, false)?;
    zipnolo::cli::execute_info_flow(&folder_archive)?;

    let _ = fs::remove_dir_all(&base_dir);
    Ok(())
}
