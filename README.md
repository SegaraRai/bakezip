# BakeZip

**[Open Web App](https://bakezip.roundtrip.dev/)**

BakeZip is a modern tool designed to fix **mojibake** (garbled filenames) in ZIP archives. It runs entirely in your browser using WebAssembly, ensuring your files never leave your device.

Common issues like ZIP files created on Windows (using legacy encodings like Shift-JIS or CP437) displaying incorrectly on macOS or Linux (which expect UTF-8) are solved instantly. BakeZip detects the correct encoding and rebuilds the archive with proper UTF-8 flags for maximum compatibility.

## Features

- **Fix Garbled Filenames**: Automatically detects and corrects character encodings (Shift-JIS, GBK, EUC-KR, etc.).
- **100% Private**: Powered by Rust and WebAssembly, all processing happens locally in your browser. No file uploads.
- **Compatibility Analysis**: Inspects archives to identify potential cross-platform issues.
- **Standard Compliant**: Rebuilds ZIPs with correct UTF-8 flags and Unicode path extra fields.

## Development

BakeZip is built with a Rust core compiled to WebAssembly, wrapped in a modern web interface.

- **Core**: Rust (ZIP parsing/writing, encoding detection)
- **Frontend**: Astro, Svelte, TailwindCSS
- **WASM**: wasm-bindgen

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
