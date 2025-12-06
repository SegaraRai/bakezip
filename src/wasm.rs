use crate::zip::parse::{ZipFile, ZipReader};
use crate::zip::transcode::{DecodeConfig, decode_filename};
use js_sys::Uint8Array;
use serde::{Deserialize, Serialize};
use std::io;
use tsify::Tsify;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::File;

#[derive(Debug, Clone, Serialize, Deserialize, Tsify)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct WasmZipFile {
    entries: Vec<WasmZipEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Tsify)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct WasmZipEntry {
    filename: Option<String>,
    encoding: Option<String>,
    original_bytes: Vec<u8>,
    compressed_size: u32,
    uncompressed_size: u32,
}

/// WASM reader that streams data from a JavaScript File without buffering the entire file
pub struct WasmFileReader {
    file: File,
}

impl ZipReader for WasmFileReader {
    fn read(
        &mut self,
        offset: u64,
        size: u64,
    ) -> impl std::future::Future<Output = Result<Vec<u8>, io::Error>> {
        read_file_slice(&self.file, offset, size)
    }
}

// Helper function to read a slice of the file
// This is needed to avoid keeping JS objects across await points
async fn read_file_slice(file: &File, offset: u64, size: u64) -> io::Result<Vec<u8>> {
    let blob = file
        .slice_with_i32_and_i32(offset as i32, (offset + size) as i32)
        .map_err(|_| io::Error::new(io::ErrorKind::Other, "Failed to slice file"))?;

    let array_buffer_promise = blob.array_buffer();
    let array_buffer = JsFuture::from(array_buffer_promise)
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::Other, "Failed to read blob"))?;

    let typed_array = Uint8Array::new(&array_buffer);
    Ok(typed_array.to_vec())
}

#[wasm_bindgen]
pub async fn parse_zip_file(file: File) -> Result<WasmZipFile, JsValue> {
    // Get file size without reading the entire file
    let file_size = file.size() as u64;

    let mut reader = WasmFileReader { file };

    // Parse the zip file
    let zip_file = parse_zip(&mut reader, file_size)
        .await
        .map_err(|e| JsValue::from_str(&format!("Failed to parse zip: {:?}", e)))?;

    // Convert to WASM-friendly format
    let entries = zip_file
        .entries
        .iter()
        .map(|(cdh, _lfh)| {
            // Decode filename with default config (auto-detect)
            let config = DecodeConfig::default();
            let decoded = decode_filename(&cdh.file_name, &config);

            WasmZipEntry {
                filename: decoded.filename,
                encoding: decoded.encoding_used.map(|e| e.to_string()),
                original_bytes: decoded.original_bytes,
                compressed_size: cdh.compressed_size,
                uncompressed_size: cdh.uncompressed_size,
            }
        })
        .collect();

    Ok(WasmZipFile { entries })
}

#[wasm_bindgen]
pub async fn parse_zip_file_with_config(
    file: File,
    encoding: Option<String>,
    prefer_extra_field: bool,
) -> Result<WasmZipFile, JsValue> {
    // Get file size without reading the entire file
    let file_size = file.size() as u64;

    let mut reader = WasmFileReader { file };

    // Parse the zip file
    let zip_file = ZipFile::parse(&mut reader, file_size)
        .await
        .map_err(|e| JsValue::from_str(&format!("Failed to parse zip: {:?}", e)))?;

    // Parse encoding preference
    let encoding = encoding
        .as_deref()
        .and_then(|enc| match enc.to_lowercase().as_str() {
            "utf8" | "utf-8" => Some(crate::zip::transcode::CharacterEncoding::Utf8),
            "utf16le" | "utf-16le" => Some(crate::zip::transcode::CharacterEncoding::Utf16Le),
            "utf16be" | "utf-16be" => Some(crate::zip::transcode::CharacterEncoding::Utf16Be),
            "cp932" | "shift_jis" => Some(crate::zip::transcode::CharacterEncoding::Cp932),
            _ => None,
        });

    let config = DecodeConfig {
        encoding,
        prefer_extra_field,
    };

    // Convert to WASM-friendly format
    let entries = zip_file
        .entries
        .iter()
        .map(|(cdh, _lfh)| {
            let decoded = decode_filename(&cdh.file_name, &config);

            WasmZipEntry {
                filename: decoded.filename,
                encoding: decoded.encoding_used.map(|e| e.to_string()),
                original_bytes: decoded.original_bytes,
                compressed_size: cdh.compressed_size,
                uncompressed_size: cdh.uncompressed_size,
            }
        })
        .collect();

    Ok(WasmZipFile { entries })
}
