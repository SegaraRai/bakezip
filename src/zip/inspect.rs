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
    /// Encoding selection strategy for filename decoding
    pub encoding: EncodingSelectionStrategy,
    /// Field selection strategy for filename decoding
    pub field_selection_strategy: FieldSelectionStrategy,
    /// Whether to ignore CRC32 mismatches in UTF-8 extra fields
    ///
    /// Should be true only when the archive is known to be created by a broken implementation
    pub ignore_crc32_mismatch: bool,
    /// Whether to require original bytes for inspected filename fields
    pub needs_original_bytes: bool,
    /// How to handle Wave Dash (U+301C) when decoding from Shift_JIS
    #[serde(default)]
    pub wave_dash_handling: WaveDashHandling,
    /// How to normalize Wave Dash (U+301C) and Fullwidth Tilde (U+FF5E)
    #[serde(default)]
    pub wave_dash_normalization: WaveDashNormalization,
}

/// Strategy for handling Wave Dash when decoding from Shift_JIS
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    derive(tsify::Tsify)
)]
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    tsify(into_wasm_abi, from_wasm_abi)
)]
pub enum WaveDashHandling {
    #[default]
    DecodeToFullwidthTilde,
    DecodeToWaveDash,
}

/// Strategy for normalizing Wave Dash and Fullwidth Tilde
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    derive(tsify::Tsify)
)]
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    tsify(into_wasm_abi, from_wasm_abi)
)]
pub enum WaveDashNormalization {
    #[default]
    Preserve,
    NormalizeToFullwidthTilde,
    NormalizeToWaveDash,
}

/// Strategy for selecting which filename field to use
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    derive(tsify::Tsify)
)]
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    tsify(into_wasm_abi, from_wasm_abi)
)]
pub enum FieldSelectionStrategy {
    #[default]
    CdhUnicodeThenLfhUnicodeThenCdh,
    CdhUnicodeThenLfhUnicodeThenLfh,
    LfhUnicodeThenCdhUnicodeThenCdh,
    LfhUnicodeThenCdhUnicodeThenLfh,
    CdhUnicodeThenCdh,
    CdhOnly,
    LfhUnicodeThenLfh,
    LfhOnly,
}

/// Strategy for selecting encoding
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    derive(tsify::Tsify)
)]
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    tsify(into_wasm_abi, from_wasm_abi)
)]
pub enum EncodingSelectionStrategy {
    /// Use overall detected encoding if available, then try detected encoding per entry, then fallback to default
    PreferOverallDetected {
        fallback_encoding: Option<String>,
        ignore_utf8_flag: bool,
    },
    /// Use detected encoding per entry if available, then fallback to default
    EntryDetected {
        fallback_encoding: Option<String>,
        ignore_utf8_flag: bool,
    },
    /// Always use the specified encoding
    ForceSpecified {
        encoding: String,
        ignore_utf8_flag: bool,
    },
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
    /// None if detection failed or an error occurred during decoding
    /// If present, this encoding can be used to decode all filenames in the archive without errors
    pub overall_encoding: Option<String>,
    /// Whether the archive contains entries encoded in Shift_JIS that have Wave Dash or Fullwidth Tilde
    pub contains_sjis_wave_dash: bool,
    /// Whether the archive contains entries NOT encoded in Shift_JIS that have Wave Dash
    pub contains_other_wave_dash: bool,
    /// Whether the archive contains entries NOT encoded in Shift_JIS that have Fullwidth Tilde
    pub contains_other_fullwidth_tilde: bool,
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
    pub original_bytes: Option<Vec<u8>>,
    /// The detected encoding used for decoding
    pub detected_encoding: Option<String>,
    /// The decoded filename
    pub decoded: Option<DecodedString>,
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
    CdhFilename,
    /// Central Directory Header "up" Extra Field (UTF-8)
    CdhUnicodePathExtraField,
    /// Local File Header File Name Field
    LfhFilename,
    /// Local File Header "up" Extra Field (UTF-8)
    LfhUnicodePathExtraField,
}

impl InspectedArchive {
    pub fn inspect(zip_file: &ZipFile, config: &InspectConfig) -> Result<Self, ZipInspectError> {
        let fields = match config.field_selection_strategy {
            FieldSelectionStrategy::CdhUnicodeThenLfhUnicodeThenCdh => &[
                InspectedFilenameFieldKind::CdhUnicodePathExtraField,
                InspectedFilenameFieldKind::LfhUnicodePathExtraField,
                InspectedFilenameFieldKind::CdhFilename,
            ][..],
            FieldSelectionStrategy::CdhUnicodeThenLfhUnicodeThenLfh => &[
                InspectedFilenameFieldKind::CdhUnicodePathExtraField,
                InspectedFilenameFieldKind::LfhUnicodePathExtraField,
                InspectedFilenameFieldKind::LfhFilename,
            ][..],
            FieldSelectionStrategy::LfhUnicodeThenCdhUnicodeThenCdh => &[
                InspectedFilenameFieldKind::LfhUnicodePathExtraField,
                InspectedFilenameFieldKind::CdhUnicodePathExtraField,
                InspectedFilenameFieldKind::CdhFilename,
            ][..],
            FieldSelectionStrategy::LfhUnicodeThenCdhUnicodeThenLfh => &[
                InspectedFilenameFieldKind::LfhUnicodePathExtraField,
                InspectedFilenameFieldKind::CdhUnicodePathExtraField,
                InspectedFilenameFieldKind::LfhFilename,
            ][..],
            FieldSelectionStrategy::CdhUnicodeThenCdh => &[
                InspectedFilenameFieldKind::CdhUnicodePathExtraField,
                InspectedFilenameFieldKind::CdhFilename,
            ][..],
            FieldSelectionStrategy::CdhOnly => &[InspectedFilenameFieldKind::CdhFilename][..],
            FieldSelectionStrategy::LfhUnicodeThenLfh => &[
                InspectedFilenameFieldKind::LfhUnicodePathExtraField,
                InspectedFilenameFieldKind::LfhFilename,
            ][..],
            FieldSelectionStrategy::LfhOnly => &[InspectedFilenameFieldKind::LfhFilename][..],
        };

        struct FieldSelectedFileEntry<'a> {
            kind: InspectedFilenameFieldKind,
            utf8_flag: bool,
            original_bytes: &'a [u8],
        }

        let predetect_entries = zip_file
            .entries
            .iter()
            .map(|entry| {
                for &kind in fields {
                    match kind {
                        InspectedFilenameFieldKind::CdhFilename => {
                            return FieldSelectedFileEntry {
                                kind,
                                utf8_flag: entry.cdh.flags.is_utf8(),
                                original_bytes: &entry.cdh.filename,
                            };
                        }
                        InspectedFilenameFieldKind::LfhFilename => {
                            return FieldSelectedFileEntry {
                                kind,
                                utf8_flag: entry.lfh.flags.is_utf8(),
                                original_bytes: &entry.lfh.filename,
                            };
                        }
                        InspectedFilenameFieldKind::CdhUnicodePathExtraField => {
                            if let Some(up) = &entry.cdh.unicode_path
                                && (config.ignore_crc32_mismatch || up.crc32_matched)
                            {
                                return FieldSelectedFileEntry {
                                    kind,
                                    utf8_flag: true,
                                    original_bytes: &up.data,
                                };
                            }
                        }
                        InspectedFilenameFieldKind::LfhUnicodePathExtraField => {
                            if let Some(up) = &entry.lfh.unicode_path
                                && (config.ignore_crc32_mismatch || up.crc32_matched)
                            {
                                return FieldSelectedFileEntry {
                                    kind,
                                    utf8_flag: true,
                                    original_bytes: &up.data,
                                };
                            }
                        }
                    }
                }

                unreachable!("At least one field should be selected")
            })
            .collect::<Vec<_>>();

        let concatenated_filename_bytes = predetect_entries
            .iter()
            .flat_map(|entry| entry.original_bytes)
            .copied()
            .collect::<Vec<_>>();
        let overall_encoding = detect_encoding(&concatenated_filename_bytes);

        let user_encoding = match &config.encoding {
            EncodingSelectionStrategy::PreferOverallDetected {
                fallback_encoding,
                ignore_utf8_flag: _,
            }
            | EncodingSelectionStrategy::EntryDetected {
                fallback_encoding,
                ignore_utf8_flag: _,
            } => {
                if let Some(enc_name) = fallback_encoding {
                    let encoding = Encoding::for_label(enc_name.as_bytes())
                        .ok_or(ZipInspectError::EncodingNotFound(enc_name.clone()))?;
                    Some(EncodingOrAscii::Encoding(encoding))
                } else {
                    None
                }
            }
            EncodingSelectionStrategy::ForceSpecified {
                encoding: enc_name,
                ignore_utf8_flag: _,
            } => {
                let encoding = Encoding::for_label(enc_name.as_bytes())
                    .ok_or(ZipInspectError::EncodingNotFound(enc_name.clone()))?;
                Some(EncodingOrAscii::Encoding(encoding))
            }
        };

        let ignore_utf8_flag = match &config.encoding {
            EncodingSelectionStrategy::PreferOverallDetected {
                fallback_encoding: _,
                ignore_utf8_flag,
            }
            | EncodingSelectionStrategy::EntryDetected {
                fallback_encoding: _,
                ignore_utf8_flag,
            }
            | EncodingSelectionStrategy::ForceSpecified {
                encoding: _,
                ignore_utf8_flag,
            } => *ignore_utf8_flag,
        };

        let entries = predetect_entries
            .into_iter()
            .map(|predetect| {
                let detected_encoding = detect_encoding(predetect.original_bytes);

                let encoding = if (!ignore_utf8_flag && predetect.utf8_flag)
                    || matches!(
                        predetect.kind,
                        InspectedFilenameFieldKind::CdhUnicodePathExtraField
                            | InspectedFilenameFieldKind::LfhUnicodePathExtraField
                    ) {
                    Some(EncodingOrAscii::Encoding(UTF_8))
                } else {
                    match &config.encoding {
                        EncodingSelectionStrategy::PreferOverallDetected {
                            fallback_encoding: _,
                            ignore_utf8_flag: _,
                        } => overall_encoding.or(detected_encoding).or(user_encoding),
                        EncodingSelectionStrategy::EntryDetected {
                            fallback_encoding: _,
                            ignore_utf8_flag: _,
                        } => detected_encoding.or(user_encoding),
                        EncodingSelectionStrategy::ForceSpecified {
                            encoding: _,
                            ignore_utf8_flag: _,
                        } => user_encoding,
                    }
                };

                let decoded = encoding.and_then(|enc| {
                    decode_with_encoding(
                        predetect.original_bytes,
                        enc.encoding(),
                        true,
                        config.wave_dash_handling,
                        config.wave_dash_normalization,
                    )
                    .map(|(string, has_errors, encoding_used)| DecodedString {
                        string,
                        has_errors,
                        encoding_used: encoding_used.name().to_string(),
                    })
                });

                let original_bytes = if config.needs_original_bytes {
                    Some(predetect.original_bytes.to_vec())
                } else {
                    None
                };

                InspectedFilenameField {
                    kind: predetect.kind,
                    utf8_flag: predetect.utf8_flag,
                    original_bytes,
                    detected_encoding: detected_encoding.map(|e| e.name().to_string()),
                    decoded,
                }
            })
            .zip(zip_file.entries.iter())
            .map(|(filename_field, entry)| InspectedEntry {
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
                filename: filename_field,
            })
            .collect::<Vec<_>>();

        let mut contains_sjis_wave_dash = false;
        let mut contains_other_wave_dash = false;
        let mut contains_other_fullwidth_tilde = false;

        for entry in &entries {
            if let Some(decoded) = &entry.filename.decoded {
                let has_wd = decoded.string.contains('\u{301C}');
                let has_ft = decoded.string.contains('\u{FF5E}');
                if has_wd || has_ft {
                    let encoding = &decoded.encoding_used;
                    let is_sjis = encoding == "Shift_JIS";

                    if is_sjis {
                        contains_sjis_wave_dash = true;
                    } else if has_wd {
                        contains_other_wave_dash = true;
                    } else if has_ft {
                        contains_other_fullwidth_tilde = true;
                    }
                }
            }
        }

        Ok(Self {
            overall_encoding: overall_encoding.map(|e| e.name().to_string()),
            entries,
            contains_sjis_wave_dash,
            contains_other_wave_dash,
            contains_other_fullwidth_tilde,
        })
    }
}

/// Decode bytes using a specific encoding from encoding_rs
fn decode_with_encoding(
    data: &[u8],
    encoding: &'static Encoding,
    force: bool,
    wave_dash_handling: WaveDashHandling,
    normalization: WaveDashNormalization,
) -> Option<(String, bool, &'static Encoding)> {
    let (result, encoding_used, has_errors) = encoding.decode(data);
    if force || !has_errors {
        let mut string = result.into_owned();

        // Handle Shift_JIS specific wave dash handling
        if encoding_used.name() == "Shift_JIS"
            && matches!(wave_dash_handling, WaveDashHandling::DecodeToWaveDash)
        {
            string = string.replace('\u{FF5E}', "\u{301C}");
        }

        // Apply normalization
        match normalization {
            WaveDashNormalization::NormalizeToFullwidthTilde => {
                string = string.replace('\u{301C}', "\u{FF5E}");
            }
            WaveDashNormalization::NormalizeToWaveDash => {
                string = string.replace('\u{FF5E}', "\u{301C}");
            }
            WaveDashNormalization::Preserve => {}
        }

        Some((string, has_errors, encoding_used))
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
    if let Some((_, has_errors, encoding)) = decode_with_encoding(
        data,
        detected_encoding,
        false,
        WaveDashHandling::default(),
        WaveDashNormalization::default(),
    ) && !has_errors
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

#[derive(Debug, thiserror::Error)]
#[error("Zip inspect error: {0}")]
pub enum ZipInspectError {
    #[error("Encoding '{0}' not found")]
    EncodingNotFound(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zip::parse::{
        CentralDirectoryHeader, EndOfCentralDirectory, GeneralPurposeBitFlag, LocalFileHeader,
        UnicodePathExtraField, ZipFile, ZipFileEntry,
    };

    fn create_mock_entry(
        filename: &[u8],
        utf8_flag: bool,
        unicode_path: Option<UnicodePathExtraField>,
    ) -> ZipFileEntry {
        let flags = GeneralPurposeBitFlag(if utf8_flag { 0x0800 } else { 0 });
        let cdh = CentralDirectoryHeader {
            signature: 0x02014b50,
            version_made_by: 0,
            version_needed: 0,
            flags,
            compression_method: 0,
            last_mod_time: 0,
            last_mod_date: 0,
            crc32: 0,
            compressed_size: 0,
            uncompressed_size: 0,
            filename_length: filename.len() as u16,
            extra_field_length: 0,
            file_comment_length: 0,
            disk_number_start: 0,
            internal_file_attributes: 0,
            external_file_attributes: 0,
            local_header_offset: 0,
            filename: filename.to_vec(),
            extra_fields: vec![],
            file_comment: vec![],
            zip64: None,
            unicode_path: unicode_path.clone(),
        };
        let lfh = LocalFileHeader {
            signature: 0x04034b50,
            version_needed: 0,
            flags,
            compression_method: 0,
            last_mod_time: 0,
            last_mod_date: 0,
            crc32: 0,
            compressed_size: 0,
            uncompressed_size: 0,
            filename_length: filename.len() as u16,
            extra_field_length: 0,
            filename: filename.to_vec(),
            extra_fields: vec![],
            zip64: None,
            unicode_path,
        };

        ZipFileEntry {
            cdh,
            lfh,
            descriptor: None,
            file_offset: 0,
            file_size: 0,
        }
    }

    fn create_mock_zip(entries: Vec<ZipFileEntry>) -> ZipFile {
        ZipFile {
            size: 0,
            eocd: EndOfCentralDirectory {
                signature: 0x06054b50,
                disk_number: 0,
                disk_number_with_eocd: 0,
                entries_on_disk: entries.len() as u16,
                total_entries: entries.len() as u16,
                central_directory_size: 0,
                central_directory_offset: 0,
                comment_length: 0,
                comment: vec![],
            },
            zip64_eocd: None,
            entries,
        }
    }

    #[test]
    fn test_inspect_empty_zip() {
        let zip = create_mock_zip(vec![]);
        let config = InspectConfig {
            encoding: EncodingSelectionStrategy::EntryDetected {
                fallback_encoding: None,
                ignore_utf8_flag: false,
            },
            field_selection_strategy: FieldSelectionStrategy::default(),
            ignore_crc32_mismatch: false,
            needs_original_bytes: false,
            wave_dash_handling: WaveDashHandling::default(),
            wave_dash_normalization: WaveDashNormalization::default(),
        };
        let result = InspectedArchive::inspect(&zip, &config);
        assert!(result.is_ok());
        let inspected = result.unwrap();
        assert_eq!(inspected.entries.len(), 0);
    }

    #[test]
    fn test_inspect_utf8_entry() {
        let entry = create_mock_entry("test.txt".as_bytes(), true, None);
        let zip = create_mock_zip(vec![entry]);
        let config = InspectConfig {
            encoding: EncodingSelectionStrategy::EntryDetected {
                fallback_encoding: None,
                ignore_utf8_flag: false,
            },
            field_selection_strategy: FieldSelectionStrategy::default(),
            ignore_crc32_mismatch: false,
            needs_original_bytes: false,
            wave_dash_handling: WaveDashHandling::default(),
            wave_dash_normalization: WaveDashNormalization::default(),
        };
        let result = InspectedArchive::inspect(&zip, &config);
        assert!(result.is_ok());
        let inspected = result.unwrap();
        assert_eq!(inspected.entries.len(), 1);
        let filename = &inspected.entries[0].filename;
        assert!(filename.utf8_flag);
        assert_eq!(filename.decoded.as_ref().unwrap().string, "test.txt");
        assert_eq!(filename.decoded.as_ref().unwrap().encoding_used, "UTF-8");
    }

    #[test]
    fn test_inspect_sjis_entry() {
        // "テスト.txt" in Shift_JIS
        let sjis_bytes = b"\x83\x65\x83\x58\x83\x67.txt";
        let entry = create_mock_entry(sjis_bytes, false, None);
        let zip = create_mock_zip(vec![entry]);
        let config = InspectConfig {
            encoding: EncodingSelectionStrategy::EntryDetected {
                fallback_encoding: Some("Shift_JIS".to_string()),
                ignore_utf8_flag: false,
            },
            field_selection_strategy: FieldSelectionStrategy::default(),
            ignore_crc32_mismatch: false,
            needs_original_bytes: false,
            wave_dash_handling: WaveDashHandling::default(),
            wave_dash_normalization: WaveDashNormalization::default(),
        };
        let result = InspectedArchive::inspect(&zip, &config);
        assert!(result.is_ok());
        let inspected = result.unwrap();
        assert_eq!(inspected.entries.len(), 1);
        let filename = &inspected.entries[0].filename;
        assert!(!filename.utf8_flag);
        assert_eq!(filename.decoded.as_ref().unwrap().string, "テスト.txt");
        assert_eq!(
            filename.decoded.as_ref().unwrap().encoding_used,
            "Shift_JIS"
        );
    }

    #[test]
    fn test_inspect_unicode_path_extra_field() {
        let unicode_path = UnicodePathExtraField {
            version: 1,
            name_crc32: 0,
            data: "unicode.txt".as_bytes().to_vec(),
            decoded_string: Some("unicode.txt".to_string()),
            crc32_matched: true,
        };
        let entry = create_mock_entry(b"ascii.txt", false, Some(unicode_path));
        let zip = create_mock_zip(vec![entry]);
        let config = InspectConfig {
            encoding: EncodingSelectionStrategy::EntryDetected {
                fallback_encoding: None,
                ignore_utf8_flag: false,
            },
            field_selection_strategy: FieldSelectionStrategy::CdhUnicodeThenCdh,
            ignore_crc32_mismatch: false,
            needs_original_bytes: false,
            wave_dash_handling: WaveDashHandling::default(),
            wave_dash_normalization: WaveDashNormalization::default(),
        };
        let result = InspectedArchive::inspect(&zip, &config);
        assert!(result.is_ok());
        let inspected = result.unwrap();
        assert_eq!(inspected.entries.len(), 1);
        let filename = &inspected.entries[0].filename;
        assert_eq!(
            filename.kind,
            InspectedFilenameFieldKind::CdhUnicodePathExtraField
        );
        assert_eq!(filename.decoded.as_ref().unwrap().string, "unicode.txt");
    }

    #[test]
    fn test_inspect_force_encoding() {
        // "テスト.txt" in Shift_JIS
        let sjis_bytes = b"\x83\x65\x83\x58\x83\x67.txt";
        let entry = create_mock_entry(sjis_bytes, false, None);
        let zip = create_mock_zip(vec![entry]);
        let config = InspectConfig {
            encoding: EncodingSelectionStrategy::ForceSpecified {
                encoding: "Shift_JIS".to_string(),
                ignore_utf8_flag: false,
            },
            field_selection_strategy: FieldSelectionStrategy::default(),
            ignore_crc32_mismatch: false,
            needs_original_bytes: false,
            wave_dash_handling: WaveDashHandling::default(),
            wave_dash_normalization: WaveDashNormalization::default(),
        };
        let result = InspectedArchive::inspect(&zip, &config);
        assert!(result.is_ok());
        let inspected = result.unwrap();
        assert_eq!(inspected.entries.len(), 1);
        let filename = &inspected.entries[0].filename;
        assert_eq!(filename.decoded.as_ref().unwrap().string, "テスト.txt");
        assert_eq!(
            filename.decoded.as_ref().unwrap().encoding_used,
            "Shift_JIS"
        );
    }

    #[test]
    fn test_sjis_wave_dash_behavior() {
        // Wave Dash in Shift_JIS is 0x8160
        let sjis_bytes = b"\x81\x60";

        // Default behavior (DecodeToFullwidthTilde)
        let config_default = InspectConfig {
            encoding: EncodingSelectionStrategy::ForceSpecified {
                encoding: "Shift_JIS".to_string(),
                ignore_utf8_flag: false,
            },
            field_selection_strategy: FieldSelectionStrategy::default(),
            ignore_crc32_mismatch: false,
            needs_original_bytes: false,
            wave_dash_handling: WaveDashHandling::DecodeToFullwidthTilde,
            wave_dash_normalization: WaveDashNormalization::Preserve,
        };

        let entry = create_mock_entry(sjis_bytes, false, None);
        let zip = create_mock_zip(vec![entry.clone()]);

        let result = InspectedArchive::inspect(&zip, &config_default).unwrap();
        let decoded = result.entries[0]
            .filename
            .decoded
            .as_ref()
            .unwrap()
            .string
            .as_str();
        assert_eq!(decoded, "\u{FF5E}"); // Fullwidth Tilde
        assert!(result.contains_sjis_wave_dash);

        // DecodeToWaveDash
        let config_wave_dash = InspectConfig {
            encoding: EncodingSelectionStrategy::ForceSpecified {
                encoding: "Shift_JIS".to_string(),
                ignore_utf8_flag: false,
            },
            field_selection_strategy: FieldSelectionStrategy::default(),
            ignore_crc32_mismatch: false,
            needs_original_bytes: false,
            wave_dash_handling: WaveDashHandling::DecodeToWaveDash,
            wave_dash_normalization: WaveDashNormalization::Preserve,
        };

        let result = InspectedArchive::inspect(&zip, &config_wave_dash).unwrap();
        let decoded = result.entries[0]
            .filename
            .decoded
            .as_ref()
            .unwrap()
            .string
            .as_str();
        assert_eq!(decoded, "\u{301C}"); // Wave Dash
        assert!(result.contains_sjis_wave_dash);
    }

    #[test]
    fn test_wave_dash_normalization() {
        // U+301C Wave Dash
        let wave_dash_str = "\u{301C}";
        let entry_wd = create_mock_entry(wave_dash_str.as_bytes(), true, None);

        // U+FF5E Fullwidth Tilde
        let fullwidth_tilde_str = "\u{FF5E}";
        let entry_ft = create_mock_entry(fullwidth_tilde_str.as_bytes(), true, None);

        let zip = create_mock_zip(vec![entry_wd, entry_ft]);

        // NormalizeToFullwidthTilde
        let config_norm_ft = InspectConfig {
            encoding: EncodingSelectionStrategy::EntryDetected {
                fallback_encoding: None,
                ignore_utf8_flag: false,
            },
            field_selection_strategy: FieldSelectionStrategy::default(),
            ignore_crc32_mismatch: false,
            needs_original_bytes: false,
            wave_dash_handling: WaveDashHandling::default(),
            wave_dash_normalization: WaveDashNormalization::NormalizeToFullwidthTilde,
        };

        let result = InspectedArchive::inspect(&zip, &config_norm_ft).unwrap();
        assert_eq!(
            result.entries[0].filename.decoded.as_ref().unwrap().string,
            "\u{FF5E}"
        );
        assert_eq!(
            result.entries[1].filename.decoded.as_ref().unwrap().string,
            "\u{FF5E}"
        );

        // NormalizeToWaveDash
        let config_norm_wd = InspectConfig {
            encoding: EncodingSelectionStrategy::EntryDetected {
                fallback_encoding: None,
                ignore_utf8_flag: false,
            },
            field_selection_strategy: FieldSelectionStrategy::default(),
            ignore_crc32_mismatch: false,
            needs_original_bytes: false,
            wave_dash_handling: WaveDashHandling::default(),
            wave_dash_normalization: WaveDashNormalization::NormalizeToWaveDash,
        };

        let result = InspectedArchive::inspect(&zip, &config_norm_wd).unwrap();
        assert_eq!(
            result.entries[0].filename.decoded.as_ref().unwrap().string,
            "\u{301C}"
        );
        assert_eq!(
            result.entries[1].filename.decoded.as_ref().unwrap().string,
            "\u{301C}"
        );
    }

    #[test]
    fn test_inspect_other_wave_dash() {
        // UTF-8 Wave Dash (U+301C)
        let wave_dash_str = "\u{301C}";
        let entry = create_mock_entry(wave_dash_str.as_bytes(), true, None);
        let zip = create_mock_zip(vec![entry]);

        let config = InspectConfig {
            encoding: EncodingSelectionStrategy::PreferOverallDetected {
                fallback_encoding: None,
                ignore_utf8_flag: false,
            },
            field_selection_strategy: FieldSelectionStrategy::default(),
            ignore_crc32_mismatch: false,
            needs_original_bytes: false,
            wave_dash_handling: WaveDashHandling::default(),
            wave_dash_normalization: WaveDashNormalization::Preserve,
        };

        let result = InspectedArchive::inspect(&zip, &config).unwrap();

        assert!(result.contains_other_wave_dash);
        assert!(!result.contains_other_fullwidth_tilde);
        assert!(!result.contains_sjis_wave_dash);
    }

    #[test]
    fn test_inspect_other_fullwidth_tilde() {
        // UTF-8 Wave Dash (U+301C)
        let wave_dash_str = "\u{FF5E}";
        let entry = create_mock_entry(wave_dash_str.as_bytes(), true, None);
        let zip = create_mock_zip(vec![entry]);

        let config = InspectConfig {
            encoding: EncodingSelectionStrategy::PreferOverallDetected {
                fallback_encoding: None,
                ignore_utf8_flag: false,
            },
            field_selection_strategy: FieldSelectionStrategy::default(),
            ignore_crc32_mismatch: false,
            needs_original_bytes: false,
            wave_dash_handling: WaveDashHandling::default(),
            wave_dash_normalization: WaveDashNormalization::Preserve,
        };

        let result = InspectedArchive::inspect(&zip, &config).unwrap();

        assert!(result.contains_other_fullwidth_tilde);
        assert!(!result.contains_other_wave_dash);
        assert!(!result.contains_sjis_wave_dash);
    }
}
