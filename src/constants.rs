pub const START_MAGIC: &[u8; 4] = b"NOLO";
pub const END_MAGIC: &[u8; 4] = b"OLON";
pub const FOLDER_CONTAINER_MAGIC: &[u8; 4] = b"NLDR";
pub const FOLDER_NAME_PREFIX: &str = "NOLO_DIR:";
pub const DEFAULT_CHUNK_SIZE: usize = 4 * 1024 * 1024; // 4 MB

#[cfg(target_os = "macos")]
pub const COMPRESS_KEY_LABEL: &str = "Cmd+Enter / c";
#[cfg(not(target_os = "macos"))]
pub const COMPRESS_KEY_LABEL: &str = "Ctrl+Enter / Alt+Enter / c";
