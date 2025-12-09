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
    /// The overall detected encoding for the archive
    pub overall_encoding: Option<String>,
    /// The decoded entries
    pub entries: Vec<InspectedEntry>,
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
        fn get_utf8_extra_field<'a>(
            extra_fields: &'a [super::parse::ExtraField],
            original_filename_bytes: &'a [u8],
            ignore_crc32_mismatch: bool,
        ) -> Option<&'a [u8]> {
            extra_fields.iter().find(|f| f.tag == 0x7075).and_then(|f| {
                if f.data.len() < 5 {
                    // UTF-8 extra field too short
                    return None;
                }

                if f.data[0] != 1 {
                    // Unsupported version
                    return None;
                }

                if !ignore_crc32_mismatch {
                    let crc32_stored =
                        u32::from_le_bytes([f.data[1], f.data[2], f.data[3], f.data[4]]);
                    let crc32_computed = crc_fast::checksum(
                        crc_fast::CrcAlgorithm::Crc32IsoHdlc,
                        original_filename_bytes,
                    ) as u32;
                    if crc32_stored != crc32_computed {
                        // CRC32 mismatch, meaning the filename field may be updated by an archive tool that doesn't update the extra field
                        return None;
                    }
                }

                Some(&f.data[5..])
            })
        }

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
                    if config.ignore_extra_fields {
                        None
                    } else {
                        get_utf8_extra_field(
                            &entry.lfh.extra_fields,
                            &entry.lfh.file_name,
                            config.ignore_crc32_mismatch,
                        )
                    }
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
                    if config.ignore_extra_fields {
                        None
                    } else {
                        get_utf8_extra_field(
                            &entry.cdh.extra_fields,
                            &entry.cdh.file_name,
                            config.ignore_crc32_mismatch,
                        )
                    }
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
        let overall_encoding = auto_detect_and_decode(&concatenated_filename_bytes).map(|(_, e)| e);

        let additional_encoding = config
            .additional_encoding
            .as_ref()
            .and_then(|enc_name| Encoding::for_label(enc_name.as_bytes()))
            .map(EncodingOrAscii::Encoding);

        let entries = predetected_entries
            .into_iter()
            .map(|predetected| {
                let detected_encoding = auto_detect_and_decode(predetected.original_bytes)
                    .map(|(_, encoding)| encoding);

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
            .map(|filename_field| InspectedEntry {
                filename: filename_field,
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
/// Returns decoded string and detected encoding or None if decoding failed
fn auto_detect_and_decode(data: &[u8]) -> Option<(String, EncodingOrAscii)> {
    // Try UTF-8 (most common for modern zips)
    if let Some((decoded, _, _)) = decode_with_encoding(data, UTF_8, false)
        && !decoded.contains('\0')
    {
        let encoding = if data.iter().all(|&b| b.is_ascii()) {
            EncodingOrAscii::Ascii
        } else {
            EncodingOrAscii::Encoding(UTF_8)
        };
        return Some((decoded, encoding));
    }

    // Use chardetng for general encoding detection
    let mut detector = EncodingDetector::new();
    detector.feed(data, true);

    let detected_encoding = detector.guess(None, true);
    if let Some(decoded) = decode_with_encoding(data, detected_encoding, true) {
        return Some((decoded.0, EncodingOrAscii::Encoding(detected_encoding)));
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
