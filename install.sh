#!/usr/bin/env bash
set -euo pipefail

OS="$(uname -s)"
ARCH="$(uname -m)"
echo "==> Building zipnolo v0.2.0 for ${OS} (${ARCH})..."

if ! command -v cargo >/dev/null 2>&1; then
    echo "Error: Rust/Cargo is not installed. Install it via: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

cargo build --release

INSTALL_DIR="${HOME}/.cargo/bin"
if [ ! -d "${INSTALL_DIR}" ]; then
    mkdir -p "${HOME}/.local/bin"
    INSTALL_DIR="${HOME}/.local/bin"
fi

cp target/release/zipnolo "${INSTALL_DIR}/zipnolo"
cp target/release/zipnolo "${INSTALL_DIR}/nolozip"
chmod +x "${INSTALL_DIR}/zipnolo" "${INSTALL_DIR}/nolozip"

echo "==> Installed zipnolo and nolozip to ${INSTALL_DIR}"
echo "==> Run 'zipnolo' from any directory in your terminal!"
