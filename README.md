# adiutor-pdf

A small, fully offline desktop app for PDF page operations: merge, split, reorder and delete pages.

**Status:** in development.

## License

Dual-licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.

## Development

PDFium is loaded at runtime. Fetch a prebuilt copy (platform is one of `linux-x64`, `win-x64`, `mac-x64`, `mac-arm64`):

```sh
scripts/fetch-pdfium.sh linux-x64 pdfium
export ADIUTOR_PDFIUM_DIR="$PWD/pdfium/lib"   # pdfium/bin on Windows
cargo run
```

`ADIUTOR_PDFIUM_DIR` names the folder holding the PDFium library (`libpdfium.so`, `pdfium.dll` or `libpdfium.dylib`).
