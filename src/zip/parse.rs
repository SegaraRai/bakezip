use std::io;

/// Trait for reading data from a zip file or other source
pub trait ZipReader {
    /// Read data at the given offset and size
    fn read(
        &mut self,
        offset: u64,
        size: u64,
    ) -> impl std::future::Future<Output = Result<Vec<u8>, io::Error>>;
}

/// Parsed zip file
#[derive(Debug, Clone)]
pub struct ZipFile {
    pub entries: Vec<ZipFileEntry>,
    pub eocd: EndOfCentralDirectory,
    pub eocd_zip64: Option<(
        Zip64EndOfCentralDirectory,
        Option<Zip64EndOfCentralDirectoryLocator>,
    )>,
}

impl ZipFile {
    const ZIP64_FALLBACK_SEARCH_SIZE: u64 = 1024 * 1024; // 1 MiB

    /// Parse a complete zip file
    pub async fn parse<R: ZipReader>(reader: &mut R, file_size: u64) -> Result<Self, io::Error> {
        // Find EOCD by reading backwards from the end
        // EOCD is at least 22 bytes, at most 22 + 65535 (max comment length)
        let search_size = std::cmp::min(65557, file_size);
        let search_offset = file_size.saturating_sub(search_size);

        let eocd_data = reader.read(search_offset, search_size).await?;

        // Find EOCD signature by scanning backwards
        let mut eocd_offset = None;
        for i in (0..eocd_data.len().saturating_sub(21)).rev() {
            if parse_u32_le(&eocd_data[i..i + 4]) == 0x06054b50 {
                eocd_offset = Some(search_offset + i as u64);
                break;
            }
        }

        let eocd_offset = eocd_offset.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("EOCD signature not found in the last {search_size} bytes"),
            )
        })?;

        // Parse EOCD
        let eocd_start_in_buffer = (eocd_offset - search_offset) as usize;
        let eocd_data_slice = &eocd_data[eocd_start_in_buffer..];
        let eocd = EndOfCentralDirectory::parse(eocd_data_slice)?;

        // Parse Zip64 EOCD if present
        let eocd_zip64 = if eocd.total_entries == 0xFFFF
            || eocd.central_directory_size == 0xFFFFFFFF
            || eocd.central_directory_offset == 0xFFFFFFFF
        {
            let zip64_eocd_locator = {
                // Read Zip64 EOCD Locator (20 bytes before EOCD)
                if eocd_offset < Zip64EndOfCentralDirectoryLocator::SIZE as u64 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "Not enough space before EOCD at offset {eocd_offset} to read Zip64 EOCD Locator"
                        ),
                    ));
                }
                let zip64_eocd_locator_data = reader
                    .read(
                        eocd_offset - Zip64EndOfCentralDirectoryLocator::SIZE as u64,
                        Zip64EndOfCentralDirectoryLocator::SIZE as u64,
                    )
                    .await?;

                // Check signature
                if parse_u32_le(&zip64_eocd_locator_data[0..4]) == 0x07064b50 {
                    let zip64_eocd_locator =
                        Zip64EndOfCentralDirectoryLocator::parse(&zip64_eocd_locator_data)?;
                    Some(zip64_eocd_locator)
                } else {
                    // No valid Zip64 EOCD Locator found
                    None
                }
            };

            let zip64_eocd_offset = if let Some(locator) = &zip64_eocd_locator {
                locator.eocd_offset
            } else {
                // Find Zip64 EOCD signature before EOCD
                let search_start = eocd_offset.saturating_sub(Self::ZIP64_FALLBACK_SEARCH_SIZE);
                let search_size = eocd_offset - search_start;
                let zip64_search_data = reader.read(search_start, search_size).await?;
                let mut zip64_eocd_offset = None;
                for i in (0..zip64_search_data.len().saturating_sub(55)).rev() {
                    if parse_u32_le(&zip64_search_data[i..i + 4]) == 0x06064b50 {
                        zip64_eocd_offset = Some(search_start + i as u64);
                        break;
                    }
                }
                zip64_eocd_offset.ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "Zip64 EOCD signature not found before EOCD within last {} bytes from EOCD at offset {eocd_offset}",
                            Self::ZIP64_FALLBACK_SEARCH_SIZE
                        ),
                    )
                })?
            };

            // Read and parse Zip64 EOCD
            let zip64_eocd_header_data = reader.read(zip64_eocd_offset, 56).await?;
            let zip64_eocd_header =
                Zip64EndOfCentralDirectoryHeader::parse(&zip64_eocd_header_data)?;

            // Read extensible data sector if present
            let extensible_data_size = zip64_eocd_header.size_of_record - 44; // 44 bytes is size without extensible data
            let extensible_data = if extensible_data_size > 0 {
                reader
                    .read(zip64_eocd_offset + 56, extensible_data_size)
                    .await?
            } else {
                Vec::new()
            };

            let zip64_eocd = Zip64EndOfCentralDirectory(zip64_eocd_header, extensible_data);
            Some((zip64_eocd, zip64_eocd_locator))
        } else {
            None
        };

        // Determine effective central directory parameters
        let (
            effective_central_directory_offset,
            effective_central_directory_size,
            effective_total_entries,
        ) = if let Some((zip64_eocd, _)) = &eocd_zip64 {
            (
                zip64_eocd.0.central_directory_offset,
                zip64_eocd.0.central_directory_size,
                zip64_eocd.0.total_entries,
            )
        } else {
            (
                eocd.central_directory_offset as u64,
                eocd.central_directory_size as u64,
                eocd.total_entries as u64,
            )
        };

        // Read central directory
        let central_dir_data = reader
            .read(
                effective_central_directory_offset,
                effective_central_directory_size,
            )
            .await?;

        // Parse all CDH entries and LFH entries
        let mut entries = Vec::new();
        let mut cdh_offset = 0;

        for idx in 0..effective_total_entries {
            // Parse CDH
            if cdh_offset + CentralDirectoryHeader::MIN_SIZE > central_dir_data.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "CDH {idx} header too short, expected at least {} bytes but got {}",
                        CentralDirectoryHeader::MIN_SIZE,
                        central_dir_data.len() - cdh_offset
                    ),
                ));
            }

            let cdh = CentralDirectoryHeader::parse(&central_dir_data[cdh_offset..])?;
            cdh_offset += cdh.len();

            // Parse LFH
            let lfh_full_data = {
                let lfh_data = reader
                    .read(
                        cdh.local_header_offset as u64,
                        LocalFileHeader::MIN_SIZE as u64,
                    )
                    .await?;
                if lfh_data.len() < LocalFileHeader::MIN_SIZE {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "LFH {idx} header too short, expected {} bytes but got {}",
                            LocalFileHeader::MIN_SIZE,
                            lfh_data.len()
                        ),
                    ));
                }

                let file_name_len = parse_u16_le(&lfh_data[26..28]) as usize;
                let extra_field_len = parse_u16_le(&lfh_data[28..30]) as usize;
                let lfh_full_size = LocalFileHeader::MIN_SIZE + file_name_len + extra_field_len;

                reader
                    .read(cdh.local_header_offset as u64, lfh_full_size as u64)
                    .await?
            };
            let lfh = LocalFileHeader::parse(&lfh_full_data, cdh.local_header_offset as u64)?;

            // Parse Data Descriptor if present
            let descriptor = if lfh.flags.has_data_descriptor() {
                let has_zip64_extension = lfh.extra_fields.iter().any(|ef| ef.tag == 0x0001)
                    || cdh.extra_fields.iter().any(|ef| ef.tag == 0x0001);
                let result = if has_zip64_extension {
                    DataDescriptor::parse_zip64(
                        reader
                            .read(
                                lfh.file_data_offset + lfh.file_data_size as u64,
                                DataDescriptor::INSPECT_SIZE_ZIP64 as u64,
                            )
                            .await?
                            .as_slice(),
                    )
                } else {
                    DataDescriptor::parse_standard(
                        reader
                            .read(
                                lfh.file_data_offset + lfh.file_data_size as u64,
                                DataDescriptor::INSPECT_SIZE_STANDARD as u64,
                            )
                            .await?
                            .as_slice(),
                    )
                };
                Some(result)
            } else {
                None
            };

            entries.push(ZipFileEntry {
                cdh,
                lfh,
                descriptor: (),
                data_offset: (),
                data_size: (),
            });
        }

        Ok(ZipFile { entries, eocd })
    }
}

#[derive(Debug, Clone)]
pub struct ZipFileEntry {
    pub cdh: CentralDirectoryHeader,
    pub lfh: LocalFileHeader,
    pub descriptor: Option<DataDescriptor>,
    pub data_offset: u64,
    pub data_size: u32,
}

#[derive(Debug, Clone, Copy)]
pub enum DataDescriptor {
    Standard {
        signature: Option<u32>,
        crc32: u32,
        compressed_size: u32,
        uncompressed_size: u32,
    },
    Zip64 {
        signature: Option<u32>,
        crc32: u32,
        compressed_size: u64,
        uncompressed_size: u64,
    },
}

/// Data Descriptor
///
/// A Data Descriptor can either have a signature or not. If it does not have a signature, there is a small possibility that the value of the first field, CRC32, coincidentally matches the signature.
/// Therefore, whether a Data Descriptor has a signature is determined not by the signature's value, but by where the section immediately following the Data Descriptor begins.
///
/// Assuming both cases, with and without a signature for the Data Descriptor, we read the 4 bytes of data immediately after the Data Descriptor and check if it is the signature of LFH, CDH, EOCD, or Zip64 EOCD sections to determine the presence or absence of a DD signature.
/// If the Zip file is packed without padding, the section immediately following the Data Descriptor would normally be LFH or CDH, and even if not, it can be assumed that EOCD or Zip64 EOCD follows.
/// If we assume there is no signature, we might read the next 4 bytes of the signature of these sections. However, the possibility of the value of those 4 bytes matching the signatures of these sections is extremely low, so we can determine it accurately.
///
/// Note that if there is any padding after the Data Descriptor, the presence or absence of the Data Descriptor signature cannot be detected, and this case will be treated as an error.
/// This application considers the information written in the CDH to be correct regardless of the Data Descriptor information, so even if parsing the Data Descriptor fails, there will be no impact.
impl DataDescriptor {
    /// Size required to inspect 32-bit Data Descriptor
    ///
    /// 20 = signature (4) + crc32 (4) + compressed size (4) + uncompressed size (4) + post-descriptor signature (4)
    ///
    /// Though it is possible to read 8 bytes after the data descriptor (in case of no signature), it is safe since
    /// a LFH, CDH, or EOCD should follow, and they all have at least 8 bytes.
    pub const INSPECT_SIZE_STANDARD: usize = 20;

    /// Size required to inspect Zip64 Data Descriptor
    /// 28 = signature (4) + crc32 (4) + compressed size (8) + uncompressed size (8) + post-descriptor signature (4)
    pub const INSPECT_SIZE_ZIP64: usize = 28;

    /// Check if the descriptor is a Zip64 descriptor
    pub fn is_zip64(&self) -> bool {
        match self {
            &DataDescriptor::Standard { .. } => false,
            &DataDescriptor::Zip64 { .. } => true,
        }
    }

    /// Get the CRC32 value
    pub fn get_crc32(&self) -> u32 {
        match self {
            &DataDescriptor::Standard { crc32, .. } => crc32,
            &DataDescriptor::Zip64 { crc32, .. } => crc32,
        }
    }

    /// Get the compressed size
    pub fn get_compressed_size(&self) -> u64 {
        match self {
            &DataDescriptor::Standard {
                compressed_size, ..
            } => compressed_size as u64,
            &DataDescriptor::Zip64 {
                compressed_size, ..
            } => compressed_size,
        }
    }

    /// Get the uncompressed size
    pub fn get_uncompressed_size(&self) -> u64 {
        match self {
            &DataDescriptor::Standard {
                uncompressed_size, ..
            } => uncompressed_size as u64,
            &DataDescriptor::Zip64 {
                uncompressed_size, ..
            } => uncompressed_size,
        }
    }

    /// Parse Data Descriptor from binary data
    pub fn parse_standard(data: &[u8]) -> Result<Self, io::Error> {
        if data.len() < Self::INSPECT_SIZE_STANDARD {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Data Descriptor data too short, expected at least {} bytes but got {}",
                    Self::INSPECT_SIZE_STANDARD,
                    data.len()
                ),
            ));
        }

        let has_signature = if Self::is_next_section_signature(parse_u32_le(&data[16..20])) {
            true
        } else if Self::is_next_section_signature(parse_u32_le(&data[12..16])) {
            false
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Unknown data after Data Descriptor, cannot determine presence of signature",
            ));
        };

        let signature = if has_signature {
            Some(parse_u32_le(&data[0..4]))
        } else {
            None
        };
        if let Some(signature) = signature
            && signature != 0x08074b50
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Invalid Data Descriptor signature, expected 0x08074b50 but got {signature:#010x}"
                ),
            ));
        }

        let content = if has_signature {
            &data[4..16]
        } else {
            &data[0..12]
        };
        let crc32 = parse_u32_le(&content[0..4]);
        let compressed_size = parse_u32_le(&content[4..8]);
        let uncompressed_size = parse_u32_le(&content[8..12]);

        Ok(DataDescriptor::Standard {
            signature,
            crc32,
            compressed_size,
            uncompressed_size,
        })
    }

    /// Parse Zip64 Data Descriptor from binary data
    pub fn parse_zip64(data: &[u8]) -> Result<Self, io::Error> {
        if data.len() < Self::INSPECT_SIZE_ZIP64 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Zip64 Data Descriptor data too short, expected at least {} bytes but got {}",
                    Self::INSPECT_SIZE_ZIP64,
                    data.len()
                ),
            ));
        }

        let has_signature = if Self::is_next_section_signature(parse_u32_le(&data[24..28])) {
            true
        } else if Self::is_next_section_signature(parse_u32_le(&data[20..24])) {
            false
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Unknown data after Zip64 Data Descriptor, cannot determine presence of signature",
            ));
        };

        let signature = if has_signature {
            Some(parse_u32_le(&data[0..4]))
        } else {
            None
        };
        if let Some(signature) = signature
            && signature != 0x08074b50
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Invalid Zip64 Data Descriptor signature, expected 0x08074b50 but got {signature:#010x}"
                ),
            ));
        }

        let content = if has_signature {
            &data[4..24]
        } else {
            &data[0..20]
        };
        let crc32 = parse_u32_le(&content[0..4]);
        let compressed_size = parse_u64_le(&content[4..12]);
        let uncompressed_size = parse_u64_le(&content[12..20]);

        Ok(DataDescriptor::Zip64 {
            signature,
            crc32,
            compressed_size,
            uncompressed_size,
        })
    }

    fn is_next_section_signature(value: u32) -> bool {
        matches!(value, 0x04034b50 | 0x02014b50 | 0x06054b50 | 0x06064b50)
    }
}

/// Central Directory Header (CDH)
#[derive(Debug, Clone)]
pub struct CentralDirectoryHeader {
    /// Central file header signature = 0x02014b50
    pub signature: u32,
    /// Version made by
    pub version_made_by: u16,
    /// Version needed to extract (minimum)
    pub version_needed: u16,
    /// General purpose bit flag
    pub flags: GeneralPurposeBitFlag,
    /// Compression method
    pub compression_method: u16,
    /// Last mod file time
    pub last_mod_time: u16,
    /// Last mod file date
    pub last_mod_date: u16,
    /// CRC-32
    pub crc32: u32,
    /// Compressed size
    pub compressed_size: u32,
    /// Uncompressed size
    pub uncompressed_size: u32,
    /// File name length
    pub file_name_length: u16,
    /// Extra field length
    pub extra_field_length: u16,
    /// File comment length
    pub file_comment_length: u16,
    /// Disk number start
    pub disk_number_start: u16,
    /// Internal file attributes
    pub internal_file_attributes: u16,
    /// External file attributes
    pub external_file_attributes: u32,
    /// Relative offset of local header
    pub local_header_offset: u32,
    /// File name
    pub file_name: Vec<u8>,
    /// Parsed extra fields
    pub extra_fields: Vec<ExtraField>,
    /// File comment
    pub file_comment: Vec<u8>,
}

impl CentralDirectoryHeader {
    /// Minimum size of CDH without variable-length fields
    pub const MIN_SIZE: usize = 46;

    /// Get total size of CDH including variable-length fields
    pub fn len(&self) -> usize {
        Self::MIN_SIZE
            + self.file_name_length as usize
            + self.extra_field_length as usize
            + self.file_comment_length as usize
    }

    /// Parse a Central Directory Header from binary data
    pub fn parse(data: &[u8]) -> Result<Self, io::Error> {
        if data.len() < Self::MIN_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "CDH data too short, expected at least {} bytes but got {}",
                    Self::MIN_SIZE,
                    data.len()
                ),
            ));
        }

        let signature = parse_u32_le(&data[0..4]);
        if signature != 0x02014b50 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Invalid CDH signature, expected 0x02014b50 but got {signature:#010x}"),
            ));
        }

        let version_made_by = parse_u16_le(&data[4..6]);
        let version_needed = parse_u16_le(&data[6..8]);
        let flags = GeneralPurposeBitFlag(parse_u16_le(&data[8..10]));
        let compression_method = parse_u16_le(&data[10..12]);
        let last_mod_time = parse_u16_le(&data[12..14]);
        let last_mod_date = parse_u16_le(&data[14..16]);
        let crc32 = parse_u32_le(&data[16..20]);
        let compressed_size = parse_u32_le(&data[20..24]);
        let uncompressed_size = parse_u32_le(&data[24..28]);
        let file_name_length = parse_u16_le(&data[28..30]) as usize;
        let extra_field_length = parse_u16_le(&data[30..32]) as usize;
        let file_comment_length = parse_u16_le(&data[32..34]) as usize;
        let disk_number_start = parse_u16_le(&data[34..36]);
        let internal_file_attributes = parse_u16_le(&data[36..38]);
        let external_file_attributes = parse_u32_le(&data[38..42]);
        let local_header_offset = parse_u32_le(&data[42..46]);

        let expected_len = 46 + file_name_length + extra_field_length + file_comment_length;
        if data.len() < expected_len {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "CDH data incomplete, expected {expected_len} bytes but got {}",
                    data.len()
                ),
            ));
        }

        let offset = 46;
        let file_name = data[offset..offset + file_name_length].to_vec();
        let offset = offset + file_name_length;
        let extra_field_data = data[offset..offset + extra_field_length].to_vec();
        let offset = offset + extra_field_length;
        let file_comment = data[offset..offset + file_comment_length].to_vec();

        let extra_fields = ExtraField::parse_all(&extra_field_data)?;

        Ok(CentralDirectoryHeader {
            signature,
            version_made_by,
            version_needed,
            flags,
            compression_method,
            last_mod_time,
            last_mod_date,
            crc32,
            compressed_size,
            uncompressed_size,
            file_name_length: file_name_length as u16,
            extra_field_length: extra_field_length as u16,
            file_comment_length: file_comment_length as u16,
            disk_number_start,
            internal_file_attributes,
            external_file_attributes,
            local_header_offset,
            file_name,
            extra_fields,
            file_comment,
        })
    }
}

/// End of Central Directory record (EOCD)
#[derive(Debug, Clone)]
pub struct EndOfCentralDirectory {
    /// End of central dir signature = 0x06054b50
    pub signature: u32,
    /// Number of this disk
    pub disk_number: u16,
    /// Number of the disk with the start of the central directory
    pub disk_number_with_eocd: u16,
    /// Number of central directory records on this disk
    pub entries_on_disk: u16,
    /// Total number of central directory records
    pub total_entries: u16,
    /// Size of the central directory
    pub central_directory_size: u32,
    /// Offset of start of central directory with respect to the starting disk number
    pub central_directory_offset: u32,
    /// ZIP file comment length
    pub comment_length: u16,
    /// ZIP file comment
    pub comment: Vec<u8>,
}

impl EndOfCentralDirectory {
    /// Minimum size of EOCD without comment
    pub const MIN_SIZE: usize = 22;

    /// Parse End of Central Directory record from binary data
    pub fn parse(data: &[u8]) -> Result<Self, io::Error> {
        if data.len() < Self::MIN_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "EOCD data too short, expected at least {} bytes but got {}",
                    Self::MIN_SIZE,
                    data.len()
                ),
            ));
        }

        let signature = parse_u32_le(&data[0..4]);
        if signature != 0x06054b50 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Invalid EOCD signature, expected 0x06054b50 but got {signature:#010x}"),
            ));
        }

        let disk_number = parse_u16_le(&data[4..6]);
        let disk_number_with_eocd = parse_u16_le(&data[6..8]);
        let entries_on_disk = parse_u16_le(&data[8..10]);
        let total_entries = parse_u16_le(&data[10..12]);
        let central_directory_size = parse_u32_le(&data[12..16]);
        let central_directory_offset = parse_u32_le(&data[16..20]);
        let comment_length = parse_u16_le(&data[20..22]) as usize;

        let expected_len = 22 + comment_length;
        if data.len() < expected_len {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "EOCD comment data incomplete, expected {expected_len} bytes but got {}",
                    data.len()
                ),
            ));
        }

        let comment = data[22..22 + comment_length].to_vec();

        Ok(EndOfCentralDirectory {
            signature,
            disk_number,
            disk_number_with_eocd,
            entries_on_disk,
            total_entries,
            central_directory_size,
            central_directory_offset,
            comment_length: comment_length as u16,
            comment,
        })
    }
}

/// Zip64 End of Central Directory Locator
#[derive(Debug, Clone, Copy)]
pub struct Zip64EndOfCentralDirectoryLocator {
    /// Zip64 end of central dir locator signature = 0x07064b50
    pub signature: u32,
    /// Number of the disk with the start of the zip64 end of central directory
    pub disk_with_eocd: u32,
    /// Relative offset of the zip64 end of central directory record
    pub eocd_offset: u64,
    /// Total number of disks
    pub total_disks: u32,
}

impl Zip64EndOfCentralDirectoryLocator {
    /// Parse size of Zip64 EOCD Locator
    pub const SIZE: usize = 20;

    /// Parse Zip64 EOCD Locator from binary data
    pub fn parse(data: &[u8]) -> Result<Self, io::Error> {
        if data.len() != Self::SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Zip64 EOCD Locator data invalid length, expected {} bytes but got {}",
                    Self::SIZE,
                    data.len()
                ),
            ));
        }

        let signature = parse_u32_le(&data[0..4]);
        if signature != 0x07064b50 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Invalid Zip64 EOCD Locator signature, expected 0x07064b50 but got {signature:#010x}"
                ),
            ));
        }

        let disk_with_eocd = parse_u32_le(&data[4..8]);
        let eocd_offset = parse_u64_le(&data[8..16]);
        let total_disks = parse_u32_le(&data[16..20]);

        Ok(Zip64EndOfCentralDirectoryLocator {
            signature,
            disk_with_eocd,
            eocd_offset,
            total_disks,
        })
    }
}

/// End of Central Directory record for Zip64 (EOCD Zip64) without extensible data sector
#[derive(Debug, Clone, Copy)]
pub struct Zip64EndOfCentralDirectoryHeader {
    /// Zip64 end of central dir signature = 0x06064b50
    pub signature: u32,
    /// Size of zip64 end of central directory record
    pub size_of_record: u64,
    /// Version made by
    pub version_made_by: u16,
    /// Version needed to extract
    pub version_needed: u16,
    /// Number of this disk
    pub disk_number: u32,
    /// Number of the disk with the start of the central directory
    pub disk_number_with_eocd: u32,
    /// Total number of entries in the central directory on this disk
    pub total_entries_on_disk: u64,
    /// Total number of entries in the central directory
    pub total_entries: u64,
    /// Size of the central directory
    pub central_directory_size: u64,
    /// Offset of start of central directory with respect to the starting disk number
    pub central_directory_offset: u64,
}

impl Zip64EndOfCentralDirectoryHeader {
    /// Size of Zip64 EOCD Header
    pub const SIZE: usize = 56;

    /// Parse Zip64 EOCD Header from binary data
    pub fn parse(data: &[u8]) -> Result<Self, io::Error> {
        if data.len() != Self::SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Zip64 EOCD Header data invalid length, expected {} bytes but got {}",
                    Self::SIZE,
                    data.len()
                ),
            ));
        }

        let signature = parse_u32_le(&data[0..4]);
        if signature != 0x06064b50 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Invalid Zip64 EOCD Header signature, expected 0x06064b50 but got {signature:#010x}"
                ),
            ));
        }

        let size_of_record = parse_u64_le(&data[4..12]);
        let version_made_by = parse_u16_le(&data[12..14]);
        let version_needed = parse_u16_le(&data[14..16]);
        let disk_number = parse_u32_le(&data[16..20]);
        let disk_number_with_eocd = parse_u32_le(&data[20..24]);
        let total_entries_on_disk = parse_u64_le(&data[24..32]);
        let total_entries = parse_u64_le(&data[32..40]);
        let central_directory_size = parse_u64_le(&data[40..48]);
        let central_directory_offset = parse_u64_le(&data[48..56]);

        Ok(Zip64EndOfCentralDirectoryHeader {
            signature,
            size_of_record,
            version_made_by,
            version_needed,
            disk_number,
            disk_number_with_eocd,
            total_entries_on_disk,
            total_entries,
            central_directory_size,
            central_directory_offset,
        })
    }
}

/// End of Central Directory record for Zip64 (EOCD Zip64) with extensible data sector
#[derive(Debug, Clone)]
pub struct Zip64EndOfCentralDirectory(
    pub Zip64EndOfCentralDirectoryHeader,
    /// Extensible data sector (not parsed, just raw data)
    pub Vec<u8>,
);

/// Local File Header (LFH)
#[derive(Debug, Clone)]
pub struct LocalFileHeader {
    /// Local file header signature = 0x04034b50
    pub signature: u32,
    /// Version needed to extract (minimum)
    pub version_needed: u16,
    /// General purpose bit flag
    pub flags: GeneralPurposeBitFlag,
    /// Compression method
    pub compression_method: u16,
    /// Last mod file time
    pub last_mod_time: u16,
    /// Last mod file date
    pub last_mod_date: u16,
    /// CRC-32
    pub crc32: u32,
    /// Compressed size
    pub compressed_size: u32,
    /// Uncompressed size
    pub uncompressed_size: u32,
    /// File name length
    pub file_name_length: u16,
    /// Extra field length
    pub extra_field_length: u16,
    /// File name
    pub file_name: Vec<u8>,
    /// Parsed extra fields
    pub extra_fields: Vec<ExtraField>,
    /// File data offset and size (not parsed, just references)
    pub file_data_offset: u64,
    pub file_data_size: u32,
}

impl LocalFileHeader {
    /// Minimum size of LFH without variable-length fields
    pub const MIN_SIZE: usize = 30;

    /// Parse a Local File Header from binary data
    pub fn parse(data: &[u8], offset: u64) -> Result<Self, io::Error> {
        if data.len() < Self::MIN_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "LFH data too short, expected at least {} bytes but got {}",
                    Self::MIN_SIZE,
                    data.len()
                ),
            ));
        }

        let signature = parse_u32_le(&data[0..4]);
        if signature != 0x04034b50 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Invalid LFH signature, expected 0x04034b50 but got {signature:#010x}"),
            ));
        }

        let version_needed = parse_u16_le(&data[4..6]);
        let flags = GeneralPurposeBitFlag(parse_u16_le(&data[6..8]));
        let compression_method = parse_u16_le(&data[8..10]);
        let last_mod_time = parse_u16_le(&data[10..12]);
        let last_mod_date = parse_u16_le(&data[12..14]);
        let crc32 = parse_u32_le(&data[14..18]);
        let compressed_size = parse_u32_le(&data[18..22]);
        let uncompressed_size = parse_u32_le(&data[22..26]);
        let file_name_length = parse_u16_le(&data[26..28]) as usize;
        let extra_field_length = parse_u16_le(&data[28..30]) as usize;

        if data.len() < 30 + file_name_length + extra_field_length {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "LFH data incomplete, expected {} bytes but got {}",
                    30 + file_name_length + extra_field_length,
                    data.len()
                ),
            ));
        }

        let file_name = data[30..30 + file_name_length].to_vec();
        let extra_field_data =
            data[30 + file_name_length..30 + file_name_length + extra_field_length].to_vec();

        let extra_fields = ExtraField::parse_all(&extra_field_data)?;

        let lfh_size = 30 + file_name_length + extra_field_length;
        let file_data_offset = offset + lfh_size as u64;

        Ok(LocalFileHeader {
            signature,
            version_needed,
            flags,
            compression_method,
            last_mod_time,
            last_mod_date,
            crc32,
            compressed_size,
            uncompressed_size,
            file_name_length: file_name_length as u16,
            extra_field_length: extra_field_length as u16,
            file_name,
            extra_fields,
            file_data_offset,
            file_data_size: compressed_size,
        })
    }
}

/// Extra field record within a LFH or CDH
#[derive(Debug, Clone)]
pub struct ExtraField {
    /// Header ID (tag)
    pub tag: u16,
    /// Size of the data field
    pub size: u16,
    /// Field data
    pub data: Vec<u8>,
}

impl ExtraField {
    /// Parse extra fields from raw bytes
    fn parse_all(data: &[u8]) -> Result<Vec<Self>, io::Error> {
        let mut fields = Vec::new();
        let mut offset = 0;

        while offset + 4 <= data.len() {
            let tag = parse_u16_le(&data[offset..offset + 2]);
            let size = parse_u16_le(&data[offset + 2..offset + 4]);
            offset += 4;

            if offset + size as usize > data.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "Extra field data incomplete for tag {tag:#06x}, expected size {size} but got {}",
                        data.len() - offset
                    ),
                ));
            }

            let field_data = data[offset..offset + size as usize].to_vec();
            offset += size as usize;

            fields.push(ExtraField {
                tag,
                size,
                data: field_data,
            });
        }

        if offset != data.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Extra field data has {} extra bytes at the end",
                    data.len() - offset
                ),
            ));
        }

        Ok(fields)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct GeneralPurposeBitFlag(pub u16);

impl GeneralPurposeBitFlag {
    /// Check if the data descriptor flag is set
    pub fn has_data_descriptor(&self) -> bool {
        (self.0 & 0x0008) != 0
    }

    /// Check if the UTF-8 encoding flag (Language encoding flag, EFS) is set
    pub fn is_utf8(&self) -> bool {
        (self.0 & 0x0800) != 0
    }
}

/// Parse a u64 little-endian integer from bytes
fn parse_u64_le(data: &[u8]) -> u64 {
    u64::from_le_bytes([
        data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
    ])
}

/// Parse a u32 little-endian integer from bytes
fn parse_u32_le(data: &[u8]) -> u32 {
    u32::from_le_bytes([data[0], data[1], data[2], data[3]])
}

/// Parse a u16 little-endian integer from bytes
fn parse_u16_le(data: &[u8]) -> u16 {
    u16::from_le_bytes([data[0], data[1]])
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockReader {
        data: Vec<u8>,
    }

    impl MockReader {
        fn new(data: Vec<u8>) -> Self {
            MockReader { data }
        }
    }

    impl ZipReader for MockReader {
        async fn read(&mut self, offset: u64, size: u64) -> Result<Vec<u8>, io::Error> {
            let offset = offset as usize;
            let size = size as usize;

            if offset >= self.data.len() {
                return Ok(Vec::new());
            }

            let end = std::cmp::min(offset + size, self.data.len());
            Ok(self.data[offset..end].to_vec())
        }
    }

    #[test]
    fn test_parse_u32_le() {
        let data = [0x12, 0x34, 0x56, 0x78];
        assert_eq!(parse_u32_le(&data), 0x78563412);
    }

    #[test]
    fn test_parse_u16_le() {
        let data = [0x12, 0x34];
        assert_eq!(parse_u16_le(&data), 0x3412);
    }

    #[test]
    fn test_parse_lfh_valid() {
        let mut lfh_data = vec![0u8; 30];
        // Set signature
        lfh_data[0..4].copy_from_slice(&0x04034b50u32.to_le_bytes());
        // Set file name and extra field lengths to 0
        lfh_data[26..28].copy_from_slice(&0u16.to_le_bytes());
        lfh_data[28..30].copy_from_slice(&0u16.to_le_bytes());

        let result = LocalFileHeader::parse(&lfh_data, 0);
        assert!(result.is_ok());
        let lfh = result.unwrap();
        assert_eq!(lfh.signature, 0x04034b50);
    }

    #[test]
    fn test_parse_cdh_valid() {
        let mut cdh_data = vec![0u8; 46];
        // Set signature
        cdh_data[0..4].copy_from_slice(&0x02014b50u32.to_le_bytes());
        // Set all lengths to 0
        cdh_data[28..30].copy_from_slice(&0u16.to_le_bytes());
        cdh_data[30..32].copy_from_slice(&0u16.to_le_bytes());
        cdh_data[32..34].copy_from_slice(&0u16.to_le_bytes());

        let result = CentralDirectoryHeader::parse(&cdh_data);
        assert!(result.is_ok());
        let cdh = result.unwrap();
        assert_eq!(cdh.signature, 0x02014b50);
    }

    #[test]
    fn test_parse_eocd_valid() {
        let mut eocd_data = vec![0u8; 22];
        // Set signature
        eocd_data[0..4].copy_from_slice(&0x06054b50u32.to_le_bytes());
        // Set comment length to 0
        eocd_data[20..22].copy_from_slice(&0u16.to_le_bytes());

        let result = EndOfCentralDirectory::parse(&eocd_data);
        assert!(result.is_ok());
        let eocd = result.unwrap();
        assert_eq!(eocd.signature, 0x06054b50);
    }

    #[test]
    fn test_parse_extra_fields() {
        let mut data = vec![0u8; 14];
        // First field: tag=0x0001, size=4, data="test"
        data[0..2].copy_from_slice(&0x0001u16.to_le_bytes());
        data[2..4].copy_from_slice(&4u16.to_le_bytes());
        data[4..8].copy_from_slice(b"test");
        // Second field: tag=0x0002, size=2, data="ab"
        data[8..10].copy_from_slice(&0x0002u16.to_le_bytes());
        data[10..12].copy_from_slice(&2u16.to_le_bytes());
        data[12..14].copy_from_slice(b"ab");

        let result = ExtraField::parse_all(&data);
        assert!(result.is_ok());
        let fields = result.unwrap();
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].tag, 0x0001);
        assert_eq!(fields[0].size, 4);
        assert_eq!(fields[0].data, b"test");
        assert_eq!(fields[1].tag, 0x0002);
        assert_eq!(fields[1].size, 2);
        assert_eq!(fields[1].data, b"ab");
    }

    #[test]
    fn test_parse_extra_fields_empty() {
        let data: Vec<u8> = vec![];
        let result = ExtraField::parse_all(&data);
        assert!(result.is_ok());
        let fields = result.unwrap();
        assert_eq!(fields.len(), 0);
    }
}
