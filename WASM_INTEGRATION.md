# BakeZip WASM Integration

## Overview

BakeZip includes WebAssembly (WASM) support for browser-based ZIP file parsing. The integration allows users to:

1. **Open ZIP files directly in the browser** - No server-side processing required
2. **Parse ZIP structures** - Extract and decode file listings
3. **Select character encoding** - Choose between auto-detection, UTF-8, UTF-16LE, UTF-16BE, and CP932 (Shift JIS)
4. **Prefer extra fields** - Optionally use UTF-8 extra field (tag 0x7075) for filename decoding
5. **View decoded file lists** - Display all files with size and compression information

## Architecture

### Rust Components

**src/wasm.rs** - WASM bindings that expose:

- `parse_zip_file(file: File) -> Result<WasmZipFile, JsValue>`
  - Parses a ZIP file with auto-detection for encoding
  - Simple, one-parameter version

- `parse_zip_file_with_config(file: File, encoding: Option<String>, prefer_extra_field: bool) -> Result<WasmZipFile, JsValue>`
  - Parses a ZIP file with custom configuration
  - Supports encoding selection: `"utf8"`, `"utf16le"`, `"utf16be"`, `"cp932"`, or `null` for auto-detection

**src/wasm.rs Types:**

- `WasmZipFile` - Container for parsed ZIP entries
- `WasmZipEntry` - Individual file entry with:
  - `filename: String` - Decoded filename
  - `compressed_size: u32` - Size when compressed
  - `uncompressed_size: u32` - Original size
  - `compression_method: u16` - Compression algorithm identifier

### Web Components

**packages/web/src/components/BakeZip.svelte** - Main UI component featuring:

- File upload with drag-and-drop support
- Encoding selection dropdown (Auto, UTF-8, UTF-16LE, UTF-16BE, CP932)
- Extra field preference checkbox
- File listing table with:
  - Filename
  - Compressed size (human-readable)
  - Uncompressed size (human-readable)
  - Compression method name
- Summary statistics (total files, total compressed, total uncompressed)

## Building

### Build WASM Module

```bash
# From project root
wasm-pack build --target web --out-dir packages/web/node_modules/bakezip
```

### Run Development Server

```bash
cd packages/web
npm install
npm run dev
```

### Build for Production

```bash
cd packages/web
npm run build
```

## Features

### 1. File Upload

- Click or drag-and-drop ZIP file
- Shows selected filename
- Only accepts `.zip` files

### 2. Encoding Detection

- **Auto Detection** (default) - Intelligently detects encoding:
  - Checks for UTF-16 null byte patterns first
  - Falls back to UTF-8 for modern ZIPs
  - Uses chardetng for other encodings like Shift JIS

- **Explicit Encoding** - Force specific encoding:
  - UTF-8 for ASCII and modern unicode
  - UTF-16LE/UTF-16BE for Windows/Mac archives
  - CP932 (Shift JIS) for Japanese archives

### 3. Extra Field Support

- Respects UTF-8 extra field (tag 0x7075)
- Format: [version:1][crc32:4][utf8_name:...]
- Checkbox to prefer extra field over main filename field

### 4. File Listing

- Table with sortable columns
- Human-readable file sizes (B, KB, MB, GB)
- Compression method names (Stored, Deflated, etc.)
- Summary statistics cards

## Technical Details

### WASM Reader Implementation

The `WasmFileReader` struct implements the `ZipReader` trait:

```rust
pub struct WasmFileReader {
    data: Vec<u8>,
}

impl ZipReader for WasmFileReader {
    fn read(&mut self, offset: u64, size: u64)
        -> impl Future<Output = Result<Vec<u8>, io::Error>> + Send {
        // Reads from in-memory buffer
    }
}
```

This allows the existing ZIP parser to work seamlessly with browser File objects.

### Supported Encodings

| Encoding          | Common Use Case                     | WASM Support                     |
| ----------------- | ----------------------------------- | -------------------------------- |
| UTF-8             | Modern ZIP files, Unicode filenames | ✅ Via encoding_rs               |
| UTF-16LE          | Windows ZIP creators                | ✅ Via encoding_rs               |
| UTF-16BE          | Big-endian systems                  | ✅ Via encoding_rs               |
| CP932 (Shift JIS) | Japanese filenames                  | ✅ Via encoding_rs               |
| Auto-detect       | Unknown encoding                    | ✅ Pattern detection + chardetng |

## Dependencies

### Rust (Cargo.toml)

- `wasm-bindgen` - WASM FFI bindings
- `wasm-bindgen-futures` - Async WASM support
- `web-sys` - Web APIs (File, Blob)
- `js-sys` - JavaScript types
- `encoding_rs` - Character encoding
- `chardetng` - Encoding detection

### Web (packages/web/package.json)

- `bakezip` - WASM package (local workspace)
- `astro` - Static site generator
- `svelte` - UI component framework
- `tailwindcss` - Styling framework
- `daisyui` - Component library

## Usage Example

```javascript
import { parse_zip_file_with_config } from "bakezip";

// Get file from input element
const file = document.querySelector('input[type="file"]').files[0];

// Parse with UTF-8 encoding, no extra field preference
const result = await parse_zip_file_with_config(file, "utf8", false);

// Access entries
result.entries.forEach((entry) => {
  console.log(`${entry.filename} (${entry.uncompressed_size} bytes)`);
});
```

## Performance

- **No network latency** - Processing happens entirely in browser
- **Efficient memory usage** - File is read once into a Vec<u8>
- **Optimized build** - Release profile uses:
  - `-O z` (optimize for size)
  - LTO (Link Time Optimization)
  - Single codegen unit

Typical WASM module size: ~1-2 MB (uncompressed)

## Limitations

- Maximum file size limited by browser memory
- No actual file extraction/decompression (metadata only)
- Single-disk ZIPs only (no split archives)
- ZIP64 handled via central directory if present

## Future Enhancements

- [ ] File extraction with decompression
- [ ] Search/filter functionality
- [ ] Export file list to CSV/JSON
- [ ] Split ZIP support
- [ ] Compression ratio visualization
- [ ] Drag-and-drop entire directories
