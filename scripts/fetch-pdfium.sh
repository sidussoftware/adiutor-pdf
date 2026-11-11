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

# sha256 of the chromium/8125 archives; bump together with the URL.
case "$platform" in
  linux-x64) sha=e1c3ce6e270cb5e0c20816b379ad9f2c1309972697c505f508de92bf5e7d6659; lib=lib/libpdfium.so ;;
  win-x64) sha=9a964261760cff52570aabf038ab346d3a5034dd8b30d0d5fe8861235d356b8b; lib=bin/pdfium.dll ;;
  mac-x64) sha=8a5193535b80c7215d8932f6647ae5097465b6dba2811602c21e120c2af112b0; lib=lib/libpdfium.dylib ;;
  mac-arm64) sha=4939bd935ac83ddc09a3a4f038bc43a19cea1660316a3667f26cb07a070e139d; lib=lib/libpdfium.dylib ;;
esac

url="https://github.com/bblanchon/pdfium-binaries/releases/download/chromium/8125/pdfium-${platform}.tgz"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
curl -fsSL -o "$tmp/pdfium.tgz" "$url"
if command -v sha256sum > /dev/null; then
  echo "$sha  $tmp/pdfium.tgz" | sha256sum -c -
else
  echo "$sha  $tmp/pdfium.tgz" | shasum -a 256 -c -
fi
mkdir -p "$dest"
tar -xzf "$tmp/pdfium.tgz" -C "$dest"
[ -f "$dest/$lib" ] || { echo "missing $dest/$lib after extraction" >&2; exit 1; }
