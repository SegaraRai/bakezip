use std::io;

/// Trait for reading data from a zip file or other source
pub trait ZipReader {
    /// Read data at the given offset and size
    fn read(
        &mut self,
        offset: u64,
        size: u64,
    ) -> impl std::future::Future<Output = Result<Vec<u8>, io::Error>> + Send;
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

/// Local File Header (LFH)
#[derive(Debug, Clone)]
pub struct LocalFileHeader {
    /// Local file header signature = 0x04034b50
    pub signature: u32,
    /// Version needed to extract (minimum)
    pub version_needed: u16,
    /// General purpose bit flag
    pub flags: u16,
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
    pub flags: u16,
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

/// Parsed zip file
pub struct ZipFile {
    pub entries: Vec<(CentralDirectoryHeader, LocalFileHeader)>,
    pub eocd: EndOfCentralDirectory,
}

/// Parse a u32 little-endian integer from bytes
fn parse_u32_le(data: &[u8]) -> u32 {
    u32::from_le_bytes([data[0], data[1], data[2], data[3]])
}

/// Parse a u16 little-endian integer from bytes
fn parse_u16_le(data: &[u8]) -> u16 {
    u16::from_le_bytes([data[0], data[1]])
}

/// Parse extra fields from raw bytes
fn parse_extra_fields(data: &[u8]) -> Result<Vec<ExtraField>, io::Error> {
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

/// Parse a Local File Header from binary data
fn parse_lfh(data: &[u8], offset: u64) -> Result<LocalFileHeader, io::Error> {
    if data.len() < 30 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "LFH data too short, expected at least 30 bytes but got {}",
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
    let flags = parse_u16_le(&data[6..8]);
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

    let extra_fields = parse_extra_fields(&extra_field_data)?;

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

/// Parse a Central Directory Header from binary data
fn parse_cdh(data: &[u8]) -> Result<CentralDirectoryHeader, io::Error> {
    if data.len() < 46 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "CDH data too short, expected at least 46 bytes but got {}",
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
    let flags = parse_u16_le(&data[8..10]);
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

    let extra_fields = parse_extra_fields(&extra_field_data)?;

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

/// Parse End of Central Directory record from binary data
fn parse_eocd(data: &[u8]) -> Result<EndOfCentralDirectory, io::Error> {
    if data.len() < 22 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "EOCD data too short, expected at least 22 bytes but got {}",
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

/// Parse a complete zip file
pub async fn parse_zip<R: ZipReader>(reader: &mut R, file_size: u64) -> Result<ZipFile, io::Error> {
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
    let eocd = parse_eocd(eocd_data_slice)?;

    // Read central directory
    let central_dir_data = reader
        .read(
            eocd.central_directory_offset as u64,
            eocd.central_directory_size as u64,
        )
        .await?;

    // Parse all CDH entries and LFH entries
    let mut entries = Vec::new();
    let mut cdh_offset = 0;

    for idx in 0..eocd.total_entries {
        // Parse CDH
        if cdh_offset + 46 > central_dir_data.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "CDH {idx} header too short, expected at least 46 bytes but got {}",
                    central_dir_data.len() - cdh_offset
                ),
            ));
        }

        let cdh = parse_cdh(&central_dir_data[cdh_offset..])?;
        let cdh_size = 46
            + cdh.file_name_length as usize
            + cdh.extra_field_length as usize
            + cdh.file_comment_length as usize;

        cdh_offset += cdh_size;

        // Parse LFH
        let lfh_data = reader.read(cdh.local_header_offset as u64, 30).await?;
        if lfh_data.len() < 30 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "LFH {idx} header too short, expected 30 bytes but got {}",
                    lfh_data.len()
                ),
            ));
        }

        let file_name_len = parse_u16_le(&lfh_data[26..28]) as usize;
        let extra_field_len = parse_u16_le(&lfh_data[28..30]) as usize;
        let lfh_full_size = 30 + file_name_len + extra_field_len;

        let lfh_full_data = reader
            .read(cdh.local_header_offset as u64, lfh_full_size as u64)
            .await?;

        let lfh = parse_lfh(&lfh_full_data, cdh.local_header_offset as u64)?;

        entries.push((cdh, lfh));
    }

    Ok(ZipFile { entries, eocd })
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

        let result = parse_lfh(&lfh_data, 0);
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

        let result = parse_cdh(&cdh_data);
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

        let result = parse_eocd(&eocd_data);
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

        let result = parse_extra_fields(&data);
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
        let result = parse_extra_fields(&data);
        assert!(result.is_ok());
        let fields = result.unwrap();
        assert_eq!(fields.len(), 0);
    }
}
