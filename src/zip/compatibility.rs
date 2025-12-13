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
                    &entry.cdh.file_name,
                    &entry.cdh.unicode_path,
                ),
                (
                    &entry.lfh.flags,
                    &entry.lfh.file_name,
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
