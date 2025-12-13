use std::collections::BTreeMap;

use chardetng::EncodingDetector;
use encoding_rs::{Encoding, UTF_8};
use serde::{Deserialize, Serialize};

use super::parse::ZipFile;

/// Configuration for inspecting ZIP archives
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    derive(tsify::Tsify)
)]
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    tsify(into_wasm_abi, from_wasm_abi)
)]
pub struct InspectConfig {
    /// Whether to ignore UTF-8 extra fields
    ///
    /// Should be true only when the archive is known to be created by a broken implementation
    pub ignore_extra_fields: bool,
    /// Whether to ignore CRC32 mismatches in UTF-8 extra fields
    ///
    /// Should be true only when the archive is known to be created by a broken implementation
    pub ignore_crc32_mismatch: bool,
    /// Whether to prefer Local File Header fields over Central Directory Header fields
    ///
    /// Should be true only when the archive is known to be created by a broken implementation
    pub prefer_lfh: bool,
    /// Additional encoding to try
    pub additional_encoding: Option<String>,
}

/// Inspected ZIP archive
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    derive(tsify::Tsify)
)]
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    tsify(into_wasm_abi, from_wasm_abi)
)]
pub struct InspectedArchive {
    /// The decoded entries
    pub entries: Vec<InspectedEntry>,
    /// The overall detected encoding for the archive
    pub overall_encoding: Option<String>,
}

/// Inspected ZIP file entry
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    derive(tsify::Tsify)
)]
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    tsify(into_wasm_abi, from_wasm_abi)
)]
pub struct InspectedEntry {
    /// The decoded filename field
    pub filename: InspectedFilenameField,
    /// The uncompressed size of the entry
    pub uncompressed_size: u64,
    /// The compressed size of the entry
    pub compressed_size: u64,
}

/// Inspected filename field
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    derive(tsify::Tsify)
)]
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    tsify(into_wasm_abi, from_wasm_abi)
)]
pub struct InspectedFilenameField {
    /// The kind of filename field
    pub kind: InspectedFilenameFieldKind,
    /// Whether the UTF-8 flag is set for this field
    pub utf8_flag: bool,
    /// The original bytes before decoding
    pub original_bytes: Vec<u8>,
    /// The detected encoding used for decoding
    pub detected_encoding: Option<String>,
    /// The decoded filename, if decoding was successful
    pub decoded_map: BTreeMap<String, DecodedString>,
}

/// Decoded string with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    derive(tsify::Tsify)
)]
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    tsify(into_wasm_abi, from_wasm_abi)
)]
pub struct DecodedString {
    /// The decoded string
    pub string: String,
    /// Whether there were decoding errors
    pub has_errors: bool,
    /// The encoding specified for decoding
    pub encoding_specified: String,
    /// The encoding used for decoding
    pub encoding_used: String,
}

/// Kind of inspected filename field
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    derive(tsify::Tsify)
)]
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    tsify(into_wasm_abi, from_wasm_abi)
)]
pub enum InspectedFilenameFieldKind {
    /// Central Directory Header File Name Field
    CdhFileName,
    /// Central Directory Header "up" Extra Field (UTF-8)
    CdhExtraFieldUtf8,
    /// Local File Header File Name Field
    LfhFileName,
    /// Local File Header "up" Extra Field (UTF-8)
    LfhExtraField,
}

impl InspectedArchive {
    pub fn inspect(zip_file: &ZipFile, config: &InspectConfig) -> Self {
        struct PreDetectInspectedFileEntry<'a> {
            kind: InspectedFilenameFieldKind,
            utf8_flag: bool,
            original_bytes: &'a [u8],
        }

        let predetected_entries = zip_file
            .entries
            .iter()
            .map(|entry| {
                if config.prefer_lfh {
                    entry
                        .lfh
                        .unicode_path
                        .as_ref()
                        .and_then(|up| {
                            if config.ignore_extra_fields
                                || (!config.ignore_crc32_mismatch && up.crc32_matched)
                            {
                                None
                            } else {
                                Some(&up.data)
                            }
                        })
                        .map_or(
                            PreDetectInspectedFileEntry {
                                kind: InspectedFilenameFieldKind::LfhFileName,
                                utf8_flag: entry.lfh.flags.is_utf8(),
                                original_bytes: &entry.lfh.file_name,
                            },
                            |data| PreDetectInspectedFileEntry {
                                kind: InspectedFilenameFieldKind::LfhExtraField,
                                utf8_flag: true,
                                original_bytes: data,
                            },
                        )
                } else {
                    entry
                        .cdh
                        .unicode_path
                        .as_ref()
                        .and_then(|up| {
                            if config.ignore_extra_fields
                                || (!config.ignore_crc32_mismatch && up.crc32_matched)
                            {
                                None
                            } else {
                                Some(&up.data)
                            }
                        })
                        .map_or(
                            PreDetectInspectedFileEntry {
                                kind: InspectedFilenameFieldKind::CdhFileName,
                                utf8_flag: entry.cdh.flags.is_utf8(),
                                original_bytes: &entry.cdh.file_name,
                            },
                            |data| PreDetectInspectedFileEntry {
                                kind: InspectedFilenameFieldKind::CdhExtraFieldUtf8,
                                utf8_flag: true,
                                original_bytes: data,
                            },
                        )
                }
            })
            .collect::<Vec<_>>();

        let concatenated_filename_bytes = predetected_entries
            .iter()
            .flat_map(|e| e.original_bytes)
            .copied()
            .collect::<Vec<_>>();
        let overall_encoding = detect_encoding(&concatenated_filename_bytes);

        let additional_encoding = config
            .additional_encoding
            .as_ref()
            .and_then(|enc_name| Encoding::for_label(enc_name.as_bytes()))
            .map(EncodingOrAscii::Encoding);

        let entries = predetected_entries
            .into_iter()
            .map(|predetected| {
                let detected_encoding = detect_encoding(predetected.original_bytes);

                let all_encodings = [&detected_encoding, &overall_encoding, &additional_encoding]
                    .into_iter()
                    .copied()
                    .flatten();

                let mut decoded_map = BTreeMap::new();
                for encoding in all_encodings {
                    if decoded_map.contains_key(encoding.name()) {
                        continue;
                    }

                    let (decoded, has_errors, encoding_used) =
                        decode_with_encoding(predetected.original_bytes, encoding.encoding(), true)
                            .unwrap();

                    decoded_map.insert(
                        encoding.name().to_string(),
                        DecodedString {
                            string: decoded,
                            has_errors,
                            encoding_specified: encoding.name().to_string(),
                            encoding_used: encoding_used.name().to_string(),
                        },
                    );
                }

                InspectedFilenameField {
                    kind: predetected.kind,
                    utf8_flag: predetected.utf8_flag,
                    original_bytes: predetected.original_bytes.to_vec(),
                    detected_encoding: detected_encoding.map(|e| e.name().to_string()),
                    decoded_map,
                }
            })
            .zip(zip_file.entries.iter())
            .map(|(filename_field, entry)| InspectedEntry {
                filename: filename_field,
                uncompressed_size: entry
                    .cdh
                    .zip64
                    .and_then(|z| z.uncompressed_size)
                    .unwrap_or(entry.cdh.uncompressed_size as u64),
                compressed_size: entry
                    .cdh
                    .zip64
                    .and_then(|z| z.compressed_size)
                    .unwrap_or(entry.cdh.compressed_size as u64),
            })
            .collect::<Vec<_>>();

        Self {
            overall_encoding: overall_encoding.map(|e| e.name().to_string()),
            entries,
        }
    }
}

/// Decode bytes using a specific encoding from encoding_rs
fn decode_with_encoding(
    data: &[u8],
    encoding: &'static Encoding,
    force: bool,
) -> Option<(String, bool, &'static Encoding)> {
    let (result, encoding_used, has_errors) = encoding.decode(data);
    if force || !has_errors {
        Some((result.into_owned(), has_errors, encoding_used))
    } else {
        None
    }
}

/// Auto-detect encoding using chardetng and encoding_rs
///
/// Returns detected encoding or None if decoding failed
fn detect_encoding(data: &[u8]) -> Option<EncodingOrAscii> {
    // Try UTF-8 (most common for modern zips)
    if let Ok(decoded) = str::from_utf8(data)
        && !decoded.contains('\0')
    {
        let encoding = if data.iter().all(|&b| b.is_ascii()) {
            EncodingOrAscii::Ascii
        } else {
            EncodingOrAscii::Encoding(UTF_8)
        };
        return Some(encoding);
    }

    // Use chardetng for general encoding detection
    let mut detector = EncodingDetector::new();
    detector.feed(data, true);

    let detected_encoding = detector.guess(None, true);
    if let Some((_, has_errors, encoding)) = decode_with_encoding(data, detected_encoding, false)
        && !has_errors
    {
        return Some(EncodingOrAscii::Encoding(encoding));
    }

    None
}

/// Encoding or ASCII marker
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EncodingOrAscii {
    Ascii,
    Encoding(&'static Encoding),
}

impl EncodingOrAscii {
    fn name(&self) -> &'static str {
        match self {
            EncodingOrAscii::Ascii => "ASCII",
            EncodingOrAscii::Encoding(enc) => enc.name(),
        }
    }

    fn encoding(&self) -> &'static Encoding {
        match self {
            EncodingOrAscii::Ascii => UTF_8,
            EncodingOrAscii::Encoding(enc) => enc,
        }
    }
}
