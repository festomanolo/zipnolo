use anyhow::{bail, Context, Result};
use std::path::PathBuf;
use zipnolo::cli::{
    execute_compress_flow, execute_decompress_flow, print_usage, run_interactive_from,
    run_self_test,
};
use zipnolo::{
    clean_input_path, default_same_dir, is_nolo_archive, Algorithm, DEFAULT_CHUNK_SIZE,
};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.is_empty() {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        return run_interactive_from(&cwd);
    }

    match args[0].as_str() {
        "-v" | "--version" | "version" => {
            println!("zipnolo v0.2.0 — Created by festomanolo (https://festomanolo.com)");
            Ok(())
        }
        "-h" | "--help" | "help" => {
            print_usage();
            Ok(())
        }
        "test" | "self-test" => run_self_test(),
        "compress" | "c" => {
            if args.len() < 2 {
                print_usage();
                bail!("Usage: zipnolo compress <SOURCE_FILE_OR_FOLDER> [DEST_FOLDER] [--algo brotli|zlib|lz4|snappy] [--chunk-mb <MB>]");
            }
            let src_path = clean_input_path(&args[1]);
            if !src_path.exists() {
                bail!("Source path not found: {}", src_path.display());
            }

            let mut dest_path: Option<PathBuf> = None;
            let mut algo = Algorithm::default();
            let mut chunk_size = DEFAULT_CHUNK_SIZE;

            let mut i = 2;
            while i < args.len() {
                match args[i].as_str() {
                    "--algo" | "-a" => {
                        i += 1;
                        let val = args.get(i).context("Missing value for --algo")?;
                        algo = Algorithm::from_name(val)?;
                    }
                    "--chunk-mb" | "-c" => {
                        i += 1;
                        let val: usize = args
                            .get(i)
                            .context("Missing value for --chunk-mb")?
                            .parse()
                            .context("Invalid integer for --chunk-mb")?;
                        if val == 0 {
                            bail!("--chunk-mb must be at least 1");
                        }
                        chunk_size = val * 1024 * 1024;
                    }
                    arg if !arg.starts_with('-') && dest_path.is_none() => {
                        dest_path = Some(clean_input_path(arg));
                    }
                    other => bail!("Unknown option '{}'", other),
                }
                i += 1;
            }

            let final_dest = dest_path.unwrap_or_else(|| default_same_dir(&src_path));
            execute_compress_flow(&src_path, &final_dest, algo, chunk_size)
        }
        "decompress" | "d" => {
            if args.len() < 2 {
                print_usage();
                bail!("Usage: zipnolo decompress <ARCHIVE.nolo> [DEST_FOLDER]");
            }
            let archive_path = clean_input_path(&args[1]);
            if !archive_path.is_file() {
                bail!("Archive file not found: {}", archive_path.display());
            }
            let final_dest = if args.len() >= 3 {
                clean_input_path(&args[2])
            } else {
                default_same_dir(&archive_path)
            };
            execute_decompress_flow(&archive_path, &final_dest)
        }
        first_arg => {
            let path = clean_input_path(first_arg);
            if path.is_dir() {
                return run_interactive_from(&path);
            } else if path.is_file() {
                let final_dest = if args.len() >= 2 {
                    clean_input_path(&args[1])
                } else {
                    default_same_dir(&path)
                };
                if is_nolo_archive(&path) {
                    return execute_decompress_flow(&path, &final_dest);
                } else {
                    return execute_compress_flow(
                        &path,
                        &final_dest,
                        Algorithm::default(),
                        DEFAULT_CHUNK_SIZE,
                    );
                }
            }
            print_usage();
            bail!(
                "Unknown command or path '{}'. Run 'zipnolo --help' for usage.",
                first_arg
            );
        }
    }
}
