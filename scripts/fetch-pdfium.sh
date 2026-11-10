#!/usr/bin/env bash
# Download prebuilt PDFium (bblanchon/pdfium-binaries) into <dest-dir>.
set -euo pipefail

platform="${1:-}"
dest="${2:-}"
case "$platform" in
  linux-x64 | win-x64 | mac-x64 | mac-arm64) ;;
  *)
    echo "usage: $0 <linux-x64|win-x64|mac-x64|mac-arm64> <dest-dir>" >&2
    exit 1
    ;;
esac
[ -n "$dest" ] || { echo "usage: $0 <platform> <dest-dir>" >&2; exit 1; }

url="https://github.com/bblanchon/pdfium-binaries/releases/download/chromium/8125/pdfium-${platform}.tgz"
mkdir -p "$dest"
curl -fsSL "$url" | tar -xz -C "$dest"
