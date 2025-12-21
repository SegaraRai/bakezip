use serde::{Deserialize, Serialize};

use super::parse::ZipFile;

/// Prevalence of a feature across entries
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    derive(tsify::Tsify)
)]
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    tsify(into_wasm_abi, from_wasm_abi)
)]
pub enum Prevalence {
    None,
    Sometimes,
    AlwaysForNonAscii,
    Always,
}

/// Compatibility information
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(tag = "type")]
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    derive(tsify::Tsify)
)]
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    tsify(into_wasm_abi, from_wasm_abi)
)]
pub enum CompatibilityLevel {
    /// The archive contains one or more entries with broken filename (e.g., invalid UTF-8 with UTF-8 flag set)
    /// that may cause compatibility issues with some ZIP tools.
    Broken,
    /// The archive contains only entries with ASCII filenames
    AsciiOnly {
        /// What extent entries have the UTF-8 flag set
        with_utf8_flags: Prevalence,
        /// What extent entries have a unicode path extra field
        with_unicode_path_fields: Prevalence,
    },
    /// The archive contains only entries with valid UTF-8 or ASCII filenames
    Utf8Only {
        /// What extent entries have the UTF-8 flag set
        with_utf8_flags: Prevalence,
        /// What extent entries have a unicode path extra field
        with_unicode_path_fields: Prevalence,
    },
    /// The archive contains an entry with non-UTF-8 filename
    Other {
        /// What extent entries have a unicode path extra field
        with_unicode_path_fields: Prevalence,
    },
}

impl CompatibilityLevel {
    /// Analyze the given ZIP file and determine its compatibility level
    pub fn analyze(zip: &ZipFile) -> Self {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum EncodingKind {
            Ascii,
            Utf8,
            Other,
        }

        let mut has_broken = false;
        let mut all_ascii = true;
        let mut all_utf8 = true;
        let mut with_utf8_flags = (Prevalence::Always, false); // (prevalence, found_any)
        let mut with_unicode_path_fields = (Prevalence::Always, false); // (prevalence, found_any)

        for entry in zip.entries.iter() {
            for (flags, filename, unicode_path) in [
                (
                    &entry.cdh.flags,
                    &entry.cdh.filename,
                    &entry.cdh.unicode_path,
                ),
                (
                    &entry.lfh.flags,
                    &entry.lfh.filename,
                    &entry.lfh.unicode_path,
                ),
            ] {
                // Detect encoding
                let encoding = if str::from_utf8(filename).is_ok() {
                    if filename.iter().all(|&b| b.is_ascii()) {
                        EncodingKind::Ascii
                    } else {
                        EncodingKind::Utf8
                    }
                } else {
                    EncodingKind::Other
                };

                // Update encoding stats
                if encoding == EncodingKind::Utf8 {
                    all_ascii = false;
                } else if encoding == EncodingKind::Other {
                    all_ascii = false;
                    all_utf8 = false;
                }

                // Check UTF-8 flag
                if flags.is_utf8() {
                    // If the UTF-8 flag is set but the filename is not valid UTF-8, it's broken
                    if encoding == EncodingKind::Other {
                        has_broken = true;
                        break;
                    }

                    with_utf8_flags = (with_utf8_flags.0, true);
                } else {
                    with_utf8_flags = (
                        with_utf8_flags.0.min(match encoding {
                            EncodingKind::Ascii => Prevalence::Always,
                            _ => Prevalence::AlwaysForNonAscii,
                        }),
                        with_utf8_flags.1,
                    );
                }

                // Check unicode path extra field
                if let Some(up) = unicode_path
                    && up.crc32_matched
                {
                    // If the field is present but its content is not valid UTF-8, it's broken
                    if up.decoded_string.is_none() {
                        has_broken = true;
                        break;
                    }

                    with_unicode_path_fields = (with_unicode_path_fields.0, true);
                } else {
                    with_unicode_path_fields = (
                        with_unicode_path_fields.0.min(match encoding {
                            EncodingKind::Ascii => Prevalence::Always,
                            _ => Prevalence::AlwaysForNonAscii,
                        }),
                        with_unicode_path_fields.1,
                    );
                }
            }
        }

        // Finalize prevalence values
        let with_utf8_flags = if with_utf8_flags.1 {
            with_utf8_flags.0
        } else {
            Prevalence::None
        };

        let with_unicode_path_fields = if with_unicode_path_fields.1 {
            with_unicode_path_fields.0
        } else {
            Prevalence::None
        };

        // Determine compatibility level
        if has_broken {
            CompatibilityLevel::Broken
        } else if all_ascii {
            CompatibilityLevel::AsciiOnly {
                with_utf8_flags,
                with_unicode_path_fields,
            }
        } else if all_utf8 {
            CompatibilityLevel::Utf8Only {
                with_utf8_flags,
                with_unicode_path_fields,
            }
        } else {
            CompatibilityLevel::Other {
                with_unicode_path_fields,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zip::parse::{
        CentralDirectoryHeader, EndOfCentralDirectory, GeneralPurposeBitFlag, LocalFileHeader,
        UnicodePathExtraField, ZipFileEntry,
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
    fn test_compatibility_ascii_only() {
        let entry = create_mock_entry(b"test.txt", false, None);
        let zip = create_mock_zip(vec![entry]);
        let compatibility = CompatibilityLevel::analyze(&zip);

        match compatibility {
            CompatibilityLevel::AsciiOnly {
                with_utf8_flags,
                with_unicode_path_fields,
            } => {
                assert_eq!(with_utf8_flags, Prevalence::None);
                assert_eq!(with_unicode_path_fields, Prevalence::None);
            }
            _ => panic!("Expected AsciiOnly, got {:?}", compatibility),
        }
    }

    #[test]
    fn test_compatibility_utf8_only() {
        let entry = create_mock_entry("テスト.txt".as_bytes(), true, None);
        let zip = create_mock_zip(vec![entry]);
        let compatibility = CompatibilityLevel::analyze(&zip);

        match compatibility {
            CompatibilityLevel::Utf8Only {
                with_utf8_flags,
                with_unicode_path_fields,
            } => {
                assert_eq!(with_utf8_flags, Prevalence::Always);
                assert_eq!(with_unicode_path_fields, Prevalence::None);
            }
            _ => panic!("Expected Utf8Only, got {:?}", compatibility),
        }
    }

    #[test]
    fn test_compatibility_broken() {
        // Invalid UTF-8 with UTF-8 flag set
        let entry = create_mock_entry(b"\x80\x81", true, None);
        let zip = create_mock_zip(vec![entry]);
        let compatibility = CompatibilityLevel::analyze(&zip);

        match compatibility {
            CompatibilityLevel::Broken => {}
            _ => panic!("Expected Broken, got {:?}", compatibility),
        }
    }

    #[test]
    fn test_compatibility_other() {
        // Non-UTF-8 without UTF-8 flag (e.g. Shift-JIS)
        // "テスト" in Shift-JIS is 83 65 83 58 83 67
        let entry = create_mock_entry(b"\x83\x65\x83\x58\x83\x67.txt", false, None);
        let zip = create_mock_zip(vec![entry]);
        let compatibility = CompatibilityLevel::analyze(&zip);

        match compatibility {
            CompatibilityLevel::Other {
                with_unicode_path_fields,
            } => {
                assert_eq!(with_unicode_path_fields, Prevalence::None);
            }
            _ => panic!("Expected Other, got {:?}", compatibility),
        }
    }

    #[test]
    fn test_compatibility_with_unicode_path() {
        let unicode_path = UnicodePathExtraField {
            version: 1,
            name_crc32: 0,
            data: "テスト.txt".as_bytes().to_vec(),
            decoded_string: Some("テスト.txt".to_string()),
            crc32_matched: true,
        };
        let entry = create_mock_entry(b"test.txt", false, Some(unicode_path));
        let zip = create_mock_zip(vec![entry]);
        let compatibility = CompatibilityLevel::analyze(&zip);

        match compatibility {
            CompatibilityLevel::AsciiOnly {
                with_utf8_flags,
                with_unicode_path_fields,
            } => {
                assert_eq!(with_utf8_flags, Prevalence::None);
                assert_eq!(with_unicode_path_fields, Prevalence::Always);
            }
            _ => panic!("Expected AsciiOnly, got {:?}", compatibility),
        }
    }
}
