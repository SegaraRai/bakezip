use std::collections::BTreeMap;

use super::parse::ZipFile;
use chardetng::EncodingDetector;
use encoding_rs::{SHIFT_JIS, UTF_8, UTF_16BE, UTF_16LE};

/// Decoded filename information
#[derive(Debug, Clone)]
pub struct DecodedFilename {
    /// The original bytes before decoding
    pub original_bytes: Vec<u8>,
    /// The original bytes before decoding
    pub original_bytes_unicode: Vec<u8>,
    /// The decoded filenames
    pub decoded_map: BTreeMap<&'static str, (String, bool)>,
}

/// Decode bytes using a specific encoding from encoding_rs
fn decode_with_encoding(
    data: &[u8],
    encoding: &'static encoding_rs::Encoding,
    force: bool,
) -> Option<(String, &'static str)> {
    let (result, encoding_used, had_errors) = encoding.decode(data);
    if force || !had_errors {
        Some((result.into_owned(), encoding_used.name()))
    } else {
        None
    }
}

/// Auto-detect encoding using chardetng and encoding_rs
fn auto_detect_and_decode(data: &[u8]) -> Option<(String, &'static str)> {
    // Try UTF-8 (most common for modern zips)
    if let Some(decoded) = decode_with_encoding(data, UTF_8, false)
        && !decoded.0.contains('\0')
    {
        if data.iter().all(|&b| b.is_ascii()) {
            return Some((decoded.0, "ASCII"));
        } else {
            return Some(decoded);
        }
    }

    // Use chardetng for general encoding detection
    let mut detector = EncodingDetector::new();
    detector.feed(data, true);

    let detected_encoding = detector.guess(None, true);
    if let Some(decoded) = decode_with_encoding(data, detected_encoding, true) {
        return Some(decoded);
    }

    None
}

/// Decode a single filename using the given configuration
pub fn decode_filename(data: &[u8], config: &DecodeConfig) -> DecodedFilename {
    let original_bytes = data.to_vec();

    let result = match config.encoding {
        None => auto_detect_and_decode(data),
        Some(enccoding) => {
            let encoding = match enccoding {
                CharacterEncoding::Utf8 => UTF_8,
                CharacterEncoding::Utf16Le => UTF_16LE,
                CharacterEncoding::Utf16Be => UTF_16BE,
                CharacterEncoding::Cp932 => SHIFT_JIS,
            };
            decode_with_encoding(data, encoding, true)
        }
    };

    let (filename, encoding_used) = match result {
        Some((s, enc)) => (Some(s), Some(enc)),
        None => (None, None),
    };

    DecodedFilename {
        filename,
        original_bytes,
        encoding_used,
    }
}

/// List all filenames in a ZipFile with the given decode configuration
pub fn list_filenames(zip_file: &ZipFile, config: &DecodeConfig) -> Vec<DecodedFilename> {
    let filenames = zip_file
        .entries
        .iter()
        .map(|(cdh, _lfh)| {
            // Determine which field to use for the filename
            let filename_bytes = if config.prefer_extra_field {
                // Check for UTF-8 extra field (tag 0x7075)
                if let Some(utf8_field) = cdh.extra_fields.iter().find(|f| f.tag == 0x7075) {
                    // UTF-8 extra field format: version (1) + crc32 (4) + utf8_name
                    if utf8_field.data.len() > 5 {
                        &utf8_field.data[5..]
                    } else {
                        &cdh.file_name
                    }
                } else {
                    &cdh.file_name
                }
            } else {
                &cdh.file_name
            };
            filename_bytes
        })
        .collect::<Vec<_>>();

    let concat_filename = filenames.iter().flat_map(|s| *s).collect::<Vec<_>>();
    let decoded = decode_filename(&concat_filename, config);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_config_default() {
        let config = DecodeConfig::default();
        assert_eq!(config.encoding, None);
        assert!(!config.prefer_extra_field);
    }

    #[test]
    fn test_decode_filename_utf8() {
        let config = DecodeConfig {
            encoding: Some(CharacterEncoding::Utf8),
            prefer_extra_field: false,
        };
        let bytes = "test.txt".as_bytes();
        let result = decode_filename(bytes, &config);
        assert_eq!(result.filename, Some("test.txt".into()));
        assert_eq!(result.encoding_used, Some("UTF-8"));
    }

    #[test]
    fn test_decode_filename_utf8_unicode() {
        let config = DecodeConfig {
            encoding: Some(CharacterEncoding::Utf8),
            prefer_extra_field: false,
        };
        let bytes = "ファイル.txt".as_bytes();
        let result = decode_filename(bytes, &config);
        assert_eq!(result.filename, Some("ファイル.txt".into()));
        assert_eq!(result.encoding_used, Some("UTF-8"));
    }

    #[test]
    fn test_auto_detect_utf8() {
        let bytes = "hello world.zip".as_bytes();
        let config = DecodeConfig {
            encoding: None,
            prefer_extra_field: false,
        };
        let result = decode_filename(bytes, &config);
        assert_eq!(result.filename, Some("hello world.zip".into()));
        assert_eq!(result.encoding_used, Some("UTF-8"));
    }

    #[test]
    fn test_decode_utf16le_explicit() {
        // "hello" in UTF-16LE
        let bytes = vec![0x68, 0x00, 0x65, 0x00, 0x6c, 0x00, 0x6c, 0x00, 0x6f, 0x00];
        let config = DecodeConfig {
            encoding: Some(CharacterEncoding::Utf16Le),
            prefer_extra_field: false,
        };
        let result = decode_filename(&bytes, &config);
        assert_eq!(result.filename, Some("hello".into()));
        assert_eq!(result.encoding_used, Some("UTF-16LE"));
    }

    #[test]
    fn test_decode_utf16be_explicit() {
        // "hi" in UTF-16BE: h=0x00 0x68, i=0x00 0x69
        let bytes = vec![0x00, 0x68, 0x00, 0x69];
        let config = DecodeConfig {
            encoding: Some(CharacterEncoding::Utf16Be),
            prefer_extra_field: false,
        };
        let result = decode_filename(&bytes, &config);
        assert_eq!(result.filename, Some("hi".into()));
        assert_eq!(result.encoding_used, Some("UTF-16BE"));
    }

    #[test]
    fn test_decode_cp932_explicit() {
        // "テスト" (test in Japanese) - needs CP932/Shift JIS encoding
        let bytes = vec![0x83, 0x65, 0x83, 0x58, 0x83, 0x67];
        let config = DecodeConfig {
            encoding: Some(CharacterEncoding::Cp932),
            prefer_extra_field: false,
        };
        let result = decode_filename(&bytes, &config);
        // Should decode to the Japanese characters or a reasonable fallback
        assert_eq!(result.filename, Some("テスト".into()));
        assert_eq!(result.encoding_used, Some("Shift_JIS"));
    }

    #[test]
    fn test_decode_filename_preserves_original_bytes() {
        let bytes = vec![0xE6, 0x97, 0xA5];
        let config = DecodeConfig::default();
        let result = decode_filename(&bytes, &config);
        assert_eq!(result.original_bytes, bytes);
    }
}
