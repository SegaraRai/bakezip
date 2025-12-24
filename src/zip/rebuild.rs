use crate::zip::parse::{Zip64EndOfCentralDirectoryHeader, ZipFileEntry};

use super::inspect::{InspectConfig, InspectedArchive, ZipInspectError};
use super::parse::{
    CentralDirectoryHeader, EndOfCentralDirectory, ExtraField, LocalFileHeader,
    UnicodePathExtraField, Zip64EndOfCentralDirectoryLocator, Zip64ExtendedInfo, ZipFile,
};
use std::collections::HashSet;
use std::io::Write;
use thiserror::Error;

/// A chunk of data in the rebuilt zip file
#[derive(Debug, Clone)]
pub enum RebuildChunk {
    Reference { offset: u64, size: u64 },
    Binary(Vec<u8>),
}

/// Rebuilds a zip file with UTF-8 filenames according to the provided configuration,
/// omitting entries specified by their indices.
pub fn rebuild(
    zip_file: &ZipFile,
    config: &InspectConfig,
    omit_entries: &[u64],
) -> Result<(Vec<RebuildChunk>, u64), RebuildError> {
    struct CentralDirectoryEntryInfo<'a> {
        lfh_offset: u64,
        filename: Vec<u8>,
        entry: &'a ZipFileEntry,
        uncompressed_size: u64,
        compressed_size: u64,
        crc32: u32,
    }

    let inspected = InspectedArchive::inspect(zip_file, config)?;
    let mut chunks = Vec::new();
    let mut current_offset = 0u64;
    let mut new_cd_entries = Vec::new();

    let omit_set: HashSet<u64> = omit_entries.iter().copied().collect();

    for (index, (entry, inspected_entry)) in zip_file
        .entries
        .iter()
        .zip(inspected.entries.iter())
        .enumerate()
    {
        if omit_set.contains(&(index as u64)) {
            continue;
        }

        let filename = inspected_entry
            .filename
            .decoded
            .as_ref()
            .map(|d| d.string.as_bytes().to_vec())
            .unwrap_or_else(|| {
                inspected_entry
                    .filename
                    .original_bytes
                    .clone()
                    .unwrap_or_default()
            });

        let uncompressed_size = entry
            .cdh
            .zip64
            .and_then(|z| z.uncompressed_size)
            .unwrap_or(entry.cdh.uncompressed_size as u64);
        let compressed_size = entry
            .cdh
            .zip64
            .and_then(|z| z.compressed_size)
            .unwrap_or(entry.cdh.compressed_size as u64);
        let crc32 = entry.cdh.crc32;
        let version_needed = entry
            .cdh
            .version_needed
            .max(entry.lfh.version_needed)
            .max(20);

        // Prepare LFH
        let mut lfh_extra_fields = entry
            .lfh
            .extra_fields
            .iter()
            .filter(|ef| ef.tag != Zip64ExtendedInfo::TAG && ef.tag != UnicodePathExtraField::TAG)
            .cloned()
            .collect::<Vec<_>>();

        let mut version_needed = version_needed;
        let mut lfh_compressed_size = compressed_size as u32;
        let mut lfh_uncompressed_size = uncompressed_size as u32;

        let need_zip64_lfh = compressed_size >= 0xFFFFFFFF || uncompressed_size >= 0xFFFFFFFF;
        if need_zip64_lfh {
            // In LFH, both compressed and uncompressed sizes must be set
            version_needed = version_needed.max(45);
            lfh_compressed_size = 0xFFFFFFFF;
            lfh_uncompressed_size = 0xFFFFFFFF;

            let mut data = Vec::new();
            data.extend_from_slice(&uncompressed_size.to_le_bytes());
            data.extend_from_slice(&compressed_size.to_le_bytes());

            lfh_extra_fields.insert(
                0,
                ExtraField {
                    tag: Zip64ExtendedInfo::TAG,
                    size: data.len() as u16,
                    data,
                },
            );
        }

        let mut flags = entry.lfh.flags;
        flags.0 |= 0x0800; // Set UTF-8 flag
        flags.0 &= !0x0008; // Clear data descriptor flag

        let lfh = LocalFileHeader {
            signature: 0x04034b50,
            version_needed,
            flags,
            compression_method: entry.lfh.compression_method,
            last_mod_time: entry.lfh.last_mod_time,
            last_mod_date: entry.lfh.last_mod_date,
            crc32,
            compressed_size: lfh_compressed_size,
            uncompressed_size: lfh_uncompressed_size,
            filename_length: filename.len() as u16,
            extra_field_length: lfh_extra_fields.iter().map(|ef| 4 + ef.size).sum(),
            filename: filename.clone(),
            extra_fields: lfh_extra_fields,
            zip64: None,        // Not used for writing
            unicode_path: None, // Not used for writing
        };

        let lfh_bytes = lfh.to_bytes()?;
        let lfh_size = lfh_bytes.len() as u64;
        chunks.push(RebuildChunk::Binary(lfh_bytes));

        let lfh_offset = current_offset;
        current_offset += lfh_size;

        chunks.push(RebuildChunk::Reference {
            offset: entry.file_offset,
            size: compressed_size,
        });
        current_offset += compressed_size;

        new_cd_entries.push(CentralDirectoryEntryInfo {
            lfh_offset,
            filename,
            entry,
            uncompressed_size,
            compressed_size,
            crc32,
        });
    }

    let total_entries = new_cd_entries.len() as u64;

    let cd_start_offset = current_offset;

    for CentralDirectoryEntryInfo {
        lfh_offset,
        filename,
        entry,
        uncompressed_size,
        compressed_size,
        crc32,
    } in new_cd_entries
    {
        let mut cdh_extra_fields = entry
            .cdh
            .extra_fields
            .iter()
            .filter(|ef| ef.tag != Zip64ExtendedInfo::TAG && ef.tag != UnicodePathExtraField::TAG)
            .cloned()
            .collect::<Vec<_>>();

        let version_made_by_os = entry.cdh.version_made_by & 0xFF00;
        let version_made_by = version_made_by_os | 63; // 6.3 (Unix)

        let mut cdh_compressed_size = compressed_size as u32;
        let mut cdh_uncompressed_size = uncompressed_size as u32;
        let mut cdh_local_header_offset = lfh_offset as u32;
        let mut version_needed = entry
            .cdh
            .version_needed
            .max(entry.lfh.version_needed)
            .max(20);

        let need_zip64_cdh = compressed_size >= 0xFFFFFFFF
            || uncompressed_size >= 0xFFFFFFFF
            || lfh_offset >= 0xFFFFFFFF;

        if need_zip64_cdh {
            version_needed = version_needed.max(45);
            let mut data = Vec::new();

            if uncompressed_size >= 0xFFFFFFFF {
                cdh_uncompressed_size = 0xFFFFFFFF;
                data.extend_from_slice(&uncompressed_size.to_le_bytes());
            }

            if compressed_size >= 0xFFFFFFFF {
                cdh_compressed_size = 0xFFFFFFFF;
                data.extend_from_slice(&compressed_size.to_le_bytes());
            }

            if lfh_offset >= 0xFFFFFFFF {
                cdh_local_header_offset = 0xFFFFFFFF;
                data.extend_from_slice(&lfh_offset.to_le_bytes());
            }

            cdh_extra_fields.insert(
                0,
                ExtraField {
                    tag: Zip64ExtendedInfo::TAG,
                    size: data.len() as u16,
                    data,
                },
            );
        }

        let mut flags = entry.cdh.flags;
        flags.0 |= 0x0800; // Set UTF-8 flag
        flags.0 &= !0x0008; // Clear data descriptor flag

        let cdh = CentralDirectoryHeader {
            signature: 0x02014b50,
            version_made_by,
            version_needed,
            flags,
            compression_method: entry.cdh.compression_method,
            last_mod_time: entry.cdh.last_mod_time,
            last_mod_date: entry.cdh.last_mod_date,
            crc32,
            compressed_size: cdh_compressed_size,
            uncompressed_size: cdh_uncompressed_size,
            filename_length: filename.len() as u16,
            extra_field_length: cdh_extra_fields.iter().map(|ef| 4 + ef.size).sum(),
            file_comment_length: entry.cdh.file_comment_length,
            disk_number_start: 0,
            internal_file_attributes: entry.cdh.internal_file_attributes,
            external_file_attributes: entry.cdh.external_file_attributes,
            local_header_offset: cdh_local_header_offset,
            filename,
            extra_fields: cdh_extra_fields,
            file_comment: entry.cdh.file_comment.clone(),
            zip64: None,
            unicode_path: None,
        };

        let cdh_bytes = cdh.to_bytes()?;
        chunks.push(RebuildChunk::Binary(cdh_bytes.clone()));
        current_offset += cdh_bytes.len() as u64;
    }

    let cd_end_offset = current_offset;
    let cd_size = cd_end_offset - cd_start_offset;

    let need_zip64_eocd =
        cd_start_offset >= 0xFFFFFFFF || cd_size >= 0xFFFFFFFF || total_entries >= 0xFFFF;

    if need_zip64_eocd {
        // Write Zip64 EOCD
        let zip64_eocd_header = Zip64EndOfCentralDirectoryHeader {
            signature: 0x06064b50,
            size_of_record: 44,
            version_made_by: 63,
            version_needed: 45,
            disk_number: 0,
            disk_number_with_eocd: 0,
            total_entries_on_disk: total_entries,
            total_entries,
            central_directory_size: cd_size,
            central_directory_offset: cd_start_offset,
        };
        let zip64_eocd_bytes = zip64_eocd_header.to_bytes()?;
        chunks.push(RebuildChunk::Binary(zip64_eocd_bytes.clone()));
        let zip64_eocd_offset = current_offset;
        current_offset += zip64_eocd_bytes.len() as u64;

        // Write Zip64 EOCD Locator
        let zip64_locator = Zip64EndOfCentralDirectoryLocator {
            signature: 0x07064b50,
            disk_with_eocd: 0,
            eocd_offset: zip64_eocd_offset,
            total_disks: 1,
        };
        let zip64_locator_bytes = zip64_locator.to_bytes()?;
        chunks.push(RebuildChunk::Binary(zip64_locator_bytes));
        current_offset += 20;
    }

    // Write EOCD
    let eocd = EndOfCentralDirectory {
        signature: 0x06054b50,
        disk_number: 0,
        disk_number_with_eocd: 0,
        entries_on_disk: if total_entries >= 0xFFFF {
            0xFFFF
        } else {
            total_entries as u16
        },
        total_entries: if total_entries >= 0xFFFF {
            0xFFFF
        } else {
            total_entries as u16
        },
        central_directory_size: if cd_size >= 0xFFFFFFFF {
            0xFFFFFFFF
        } else {
            cd_size as u32
        },
        central_directory_offset: if cd_start_offset >= 0xFFFFFFFF {
            0xFFFFFFFF
        } else {
            cd_start_offset as u32
        },
        comment_length: zip_file.eocd.comment_length,
        comment: zip_file.eocd.comment.clone(),
    };
    let eocd_bytes = eocd.to_bytes()?;
    let eocd_size = eocd_bytes.len() as u64;
    chunks.push(RebuildChunk::Binary(eocd_bytes));
    current_offset += eocd_size;

    Ok((chunks, current_offset))
}

/// Errors that can occur during the rebuild process
#[derive(Debug, Error)]
pub enum RebuildError {
    #[error("Inspection failed: {0}")]
    Inspect(#[from] ZipInspectError),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Trait for serializing zip structures to bytes
trait ZipSerialize {
    fn to_bytes(&self) -> std::io::Result<Vec<u8>>;
}

impl ZipSerialize for LocalFileHeader {
    fn to_bytes(&self) -> std::io::Result<Vec<u8>> {
        let mut w = Vec::with_capacity(
            30 + self.filename.len()
                + self
                    .extra_fields
                    .iter()
                    .map(|ef| 4 + ef.size as usize)
                    .sum::<usize>(),
        );
        w.write_all(&self.signature.to_le_bytes())?;
        w.write_all(&self.version_needed.to_le_bytes())?;
        w.write_all(&self.flags.0.to_le_bytes())?;
        w.write_all(&self.compression_method.to_le_bytes())?;
        w.write_all(&self.last_mod_time.to_le_bytes())?;
        w.write_all(&self.last_mod_date.to_le_bytes())?;
        w.write_all(&self.crc32.to_le_bytes())?;
        w.write_all(&self.compressed_size.to_le_bytes())?;
        w.write_all(&self.uncompressed_size.to_le_bytes())?;
        w.write_all(&self.filename_length.to_le_bytes())?;
        w.write_all(&self.extra_field_length.to_le_bytes())?;
        w.write_all(&self.filename)?;
        for ef in &self.extra_fields {
            w.write_all(&ef.tag.to_le_bytes())?;
            w.write_all(&ef.size.to_le_bytes())?;
            w.write_all(&ef.data)?;
        }
        Ok(w)
    }
}

impl ZipSerialize for CentralDirectoryHeader {
    fn to_bytes(&self) -> std::io::Result<Vec<u8>> {
        let mut w = Vec::with_capacity(
            46 + self.filename.len()
                + self
                    .extra_fields
                    .iter()
                    .map(|ef| 4 + ef.size as usize)
                    .sum::<usize>()
                + self.file_comment.len(),
        );
        w.write_all(&self.signature.to_le_bytes())?;
        w.write_all(&self.version_made_by.to_le_bytes())?;
        w.write_all(&self.version_needed.to_le_bytes())?;
        w.write_all(&self.flags.0.to_le_bytes())?;
        w.write_all(&self.compression_method.to_le_bytes())?;
        w.write_all(&self.last_mod_time.to_le_bytes())?;
        w.write_all(&self.last_mod_date.to_le_bytes())?;
        w.write_all(&self.crc32.to_le_bytes())?;
        w.write_all(&self.compressed_size.to_le_bytes())?;
        w.write_all(&self.uncompressed_size.to_le_bytes())?;
        w.write_all(&self.filename_length.to_le_bytes())?;
        w.write_all(&self.extra_field_length.to_le_bytes())?;
        w.write_all(&self.file_comment_length.to_le_bytes())?;
        w.write_all(&self.disk_number_start.to_le_bytes())?;
        w.write_all(&self.internal_file_attributes.to_le_bytes())?;
        w.write_all(&self.external_file_attributes.to_le_bytes())?;
        w.write_all(&self.local_header_offset.to_le_bytes())?;
        w.write_all(&self.filename)?;
        for ef in &self.extra_fields {
            w.write_all(&ef.tag.to_le_bytes())?;
            w.write_all(&ef.size.to_le_bytes())?;
            w.write_all(&ef.data)?;
        }
        w.write_all(&self.file_comment)?;
        Ok(w)
    }
}

impl ZipSerialize for Zip64EndOfCentralDirectoryHeader {
    fn to_bytes(&self) -> std::io::Result<Vec<u8>> {
        let mut w = Vec::with_capacity(56);
        w.write_all(&self.signature.to_le_bytes())?;
        w.write_all(&self.size_of_record.to_le_bytes())?;
        w.write_all(&self.version_made_by.to_le_bytes())?;
        w.write_all(&self.version_needed.to_le_bytes())?;
        w.write_all(&self.disk_number.to_le_bytes())?;
        w.write_all(&self.disk_number_with_eocd.to_le_bytes())?;
        w.write_all(&self.total_entries_on_disk.to_le_bytes())?;
        w.write_all(&self.total_entries.to_le_bytes())?;
        w.write_all(&self.central_directory_size.to_le_bytes())?;
        w.write_all(&self.central_directory_offset.to_le_bytes())?;
        Ok(w)
    }
}

impl ZipSerialize for Zip64EndOfCentralDirectoryLocator {
    fn to_bytes(&self) -> std::io::Result<Vec<u8>> {
        let mut w = Vec::with_capacity(20);
        w.write_all(&self.signature.to_le_bytes())?;
        w.write_all(&self.disk_with_eocd.to_le_bytes())?;
        w.write_all(&self.eocd_offset.to_le_bytes())?;
        w.write_all(&self.total_disks.to_le_bytes())?;
        Ok(w)
    }
}

impl ZipSerialize for EndOfCentralDirectory {
    fn to_bytes(&self) -> std::io::Result<Vec<u8>> {
        let mut w = Vec::with_capacity(22 + self.comment.len());
        w.write_all(&self.signature.to_le_bytes())?;
        w.write_all(&self.disk_number.to_le_bytes())?;
        w.write_all(&self.disk_number_with_eocd.to_le_bytes())?;
        w.write_all(&self.entries_on_disk.to_le_bytes())?;
        w.write_all(&self.total_entries.to_le_bytes())?;
        w.write_all(&self.central_directory_size.to_le_bytes())?;
        w.write_all(&self.central_directory_offset.to_le_bytes())?;
        w.write_all(&self.comment_length.to_le_bytes())?;
        w.write_all(&self.comment)?;
        Ok(w)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zip::inspect::{
        EncodingSelectionStrategy, FieldSelectionStrategy, InspectConfig, WaveDashHandling,
        WaveDashNormalization,
    };
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
    fn test_rebuild_empty_zip() {
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
        let result = rebuild(&zip, &config, &[]);
        assert!(result.is_ok());
        let (chunks, size) = result.unwrap();
        // EOCD is 22 bytes
        assert_eq!(size, 22);
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_rebuild_single_entry() {
        let entry = create_mock_entry(b"test.txt", true, None);
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
        let result = rebuild(&zip, &config, &[]);
        assert!(result.is_ok());
        let (chunks, size) = result.unwrap();
        assert!(size > 22);
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_rebuild_omit_entry() {
        let entry1 = create_mock_entry(b"test1.txt", true, None);
        let entry2 = create_mock_entry(b"test2.txt", true, None);
        let zip = create_mock_zip(vec![entry1, entry2]);
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
        // Omit the first entry (index 0)
        let result = rebuild(&zip, &config, &[0]);
        assert!(result.is_ok());
        let (_chunks, size) = result.unwrap();

        // Should contain only one entry + EOCD
        // We can't easily check the exact size without calculating it, but we can check if it's smaller than full rebuild
        let full_result = rebuild(&zip, &config, &[]).unwrap();
        assert!(size < full_result.1);
    }
}
