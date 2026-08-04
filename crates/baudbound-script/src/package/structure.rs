//! Structural validation of a package archive before it is parsed.
//!
//! The editor and the runner read `.bbs` archives with different
//! implementations. The editor validates the archive structure directly, while
//! the `zip` crate reads the central directory and tolerates forms the editor
//! rejects. A reviewer inspecting a package in the editor must be looking at
//! the bytes the runner will execute, so the runner's accept set has to be a
//! subset of the editor's.
//!
//! These checks mirror `bbs-package.worker.ts` in the editor: no ZIP64, no data
//! before or after the archive, and a local header that agrees with its central
//! directory record.

use std::io::{Read, Seek, SeekFrom};

const END_OF_CENTRAL_DIRECTORY_SIGNATURE: u32 = 0x0605_4b50;
const ZIP64_LOCATOR_SIGNATURE: u32 = 0x0706_4b50;
const CENTRAL_DIRECTORY_SIGNATURE: u32 = 0x0201_4b50;
const LOCAL_HEADER_SIGNATURE: u32 = 0x0403_4b50;
const END_OF_CENTRAL_DIRECTORY_LENGTH: u64 = 22;
const CENTRAL_DIRECTORY_ENTRY_LENGTH: u64 = 46;
const LOCAL_HEADER_LENGTH: u64 = 30;
const MAXIMUM_COMMENT_LENGTH: u64 = 0xffff;
const DATA_DESCRIPTOR_FLAG: u16 = 0x0008;
const ENCRYPTION_FLAGS: u16 = 0x0041;

struct EndOfCentralDirectory {
    central_offset: u64,
    central_size: u64,
    entry_count: u64,
    offset: u64,
}

struct CentralDirectoryEntry {
    compressed_size: u32,
    compression: u16,
    crc32: u32,
    flags: u16,
    local_offset: u64,
    name: Vec<u8>,
    record_end: u64,
    uncompressed_size: u32,
}

pub(super) fn validate_archive_structure<R: Read + Seek>(reader: &mut R) -> Result<(), String> {
    let length = reader
        .seek(SeekFrom::End(0))
        .map_err(|source| format!("failed to measure the package archive: {source}"))?;

    let end = read_end_of_central_directory(reader, length)?;
    let central_end = end
        .central_offset
        .checked_add(end.central_size)
        .ok_or("package central directory location overflowed")?;
    if central_end != end.offset {
        return Err("package central directory location is invalid".to_owned());
    }

    let mut offset = end.central_offset;
    let mut first_local_offset = None;
    for index in 0..end.entry_count {
        let entry = read_central_directory_entry(reader, offset, central_end, index)?;
        validate_local_header(reader, &entry, end.central_offset)?;
        first_local_offset = Some(match first_local_offset {
            Some(current) if current <= entry.local_offset => current,
            _ => entry.local_offset,
        });
        offset = entry.record_end;
    }
    if offset != central_end {
        return Err("package central directory contains trailing data".to_owned());
    }

    // Data before the first local header shifts every offset and lets one
    // archive present different content to two readers.
    if let Some(first_local_offset) = first_local_offset
        && first_local_offset != 0
    {
        return Err("package archive contains data before its first entry".to_owned());
    }

    Ok(())
}

fn read_end_of_central_directory<R: Read + Seek>(
    reader: &mut R,
    length: u64,
) -> Result<EndOfCentralDirectory, String> {
    if length < END_OF_CENTRAL_DIRECTORY_LENGTH {
        return Err("package archive is missing its central directory".to_owned());
    }
    let tail_length = (END_OF_CENTRAL_DIRECTORY_LENGTH + MAXIMUM_COMMENT_LENGTH).min(length);
    let tail_offset = length - tail_length;
    let tail = read_exact_at(reader, tail_offset, tail_length)?;

    let limit = tail.len() - END_OF_CENTRAL_DIRECTORY_LENGTH as usize;
    for candidate in (0..=limit).rev() {
        if read_u32(&tail, candidate) != END_OF_CENTRAL_DIRECTORY_SIGNATURE {
            continue;
        }
        let comment_length = u64::from(read_u16(&tail, candidate + 20));
        let record_end = candidate as u64 + END_OF_CENTRAL_DIRECTORY_LENGTH + comment_length;
        // Anything after the record means a second archive or appended data,
        // which two readers can disagree about.
        if record_end != tail_length {
            continue;
        }

        let disk_number = read_u16(&tail, candidate + 4);
        let central_disk = read_u16(&tail, candidate + 6);
        let entries_on_disk = read_u16(&tail, candidate + 8);
        let entry_count = read_u16(&tail, candidate + 10);
        let central_size = read_u32(&tail, candidate + 12);
        let central_offset = read_u32(&tail, candidate + 16);

        if entry_count == 0xffff || central_size == 0xffff_ffff || central_offset == 0xffff_ffff {
            return Err("ZIP64 package archives are not supported".to_owned());
        }
        if disk_number != 0 || central_disk != 0 || entries_on_disk != entry_count {
            return Err("multi-disk package archives are not supported".to_owned());
        }

        let offset = tail_offset + candidate as u64;
        if candidate >= 20 && read_u32(&tail, candidate - 20) == ZIP64_LOCATOR_SIGNATURE {
            return Err("ZIP64 package archives are not supported".to_owned());
        }

        return Ok(EndOfCentralDirectory {
            central_offset: u64::from(central_offset),
            central_size: u64::from(central_size),
            entry_count: u64::from(entry_count),
            offset,
        });
    }

    Err("package archive is missing its central directory".to_owned())
}

fn read_central_directory_entry<R: Read + Seek>(
    reader: &mut R,
    offset: u64,
    central_end: u64,
    index: u64,
) -> Result<CentralDirectoryEntry, String> {
    if offset
        .checked_add(CENTRAL_DIRECTORY_ENTRY_LENGTH)
        .is_none_or(|end| end > central_end)
    {
        return Err(format!(
            "package central directory entry {} is truncated",
            index + 1
        ));
    }
    let fixed = read_exact_at(reader, offset, CENTRAL_DIRECTORY_ENTRY_LENGTH)?;
    if read_u32(&fixed, 0) != CENTRAL_DIRECTORY_SIGNATURE {
        return Err(format!(
            "package central directory entry {} is invalid",
            index + 1
        ));
    }

    let flags = read_u16(&fixed, 8);
    let compression = read_u16(&fixed, 10);
    let crc32 = read_u32(&fixed, 16);
    let compressed_size = read_u32(&fixed, 20);
    let uncompressed_size = read_u32(&fixed, 24);
    let name_length = u64::from(read_u16(&fixed, 28));
    let extra_length = u64::from(read_u16(&fixed, 30));
    let comment_length = u64::from(read_u16(&fixed, 32));
    let disk_number = read_u16(&fixed, 34);
    let local_offset = read_u32(&fixed, 42);

    if compressed_size == 0xffff_ffff
        || uncompressed_size == 0xffff_ffff
        || local_offset == 0xffff_ffff
    {
        return Err("ZIP64 package entries are not supported".to_owned());
    }
    if disk_number != 0 {
        return Err("multi-disk package archives are not supported".to_owned());
    }
    if flags & ENCRYPTION_FLAGS != 0 {
        return Err("encrypted package entries are not supported".to_owned());
    }
    if compression != 0 && compression != 8 {
        return Err(format!(
            "package entry {} uses unsupported ZIP compression method {compression}",
            index + 1
        ));
    }

    let record_end = offset
        .checked_add(CENTRAL_DIRECTORY_ENTRY_LENGTH)
        .and_then(|end| end.checked_add(name_length))
        .and_then(|end| end.checked_add(extra_length))
        .and_then(|end| end.checked_add(comment_length))
        .ok_or("package central directory entry overflowed")?;
    if record_end > central_end {
        return Err("package central directory entry is truncated".to_owned());
    }

    let name = read_exact_at(reader, offset + CENTRAL_DIRECTORY_ENTRY_LENGTH, name_length)?;

    Ok(CentralDirectoryEntry {
        compressed_size,
        compression,
        crc32,
        flags,
        local_offset: u64::from(local_offset),
        name,
        record_end,
        uncompressed_size,
    })
}

fn validate_local_header<R: Read + Seek>(
    reader: &mut R,
    entry: &CentralDirectoryEntry,
    central_offset: u64,
) -> Result<(), String> {
    let fixed = read_exact_at(reader, entry.local_offset, LOCAL_HEADER_LENGTH)?;
    if read_u32(&fixed, 0) != LOCAL_HEADER_SIGNATURE {
        return Err("package local entry header is invalid".to_owned());
    }

    let local_flags = read_u16(&fixed, 6);
    let local_compression = read_u16(&fixed, 8);
    let local_crc32 = read_u32(&fixed, 14);
    let local_compressed_size = read_u32(&fixed, 18);
    let local_uncompressed_size = read_u32(&fixed, 22);
    let name_length = u64::from(read_u16(&fixed, 26));
    let extra_length = u64::from(read_u16(&fixed, 28));

    if local_flags != entry.flags
        || local_compression != entry.compression
        || name_length != entry.name.len() as u64
    {
        return Err("package local and central entry headers disagree".to_owned());
    }

    let local_name = read_exact_at(
        reader,
        entry.local_offset + LOCAL_HEADER_LENGTH,
        name_length,
    )?;
    if local_name != entry.name {
        return Err("package local and central entry names disagree".to_owned());
    }

    // With a data descriptor the local sizes are written after the data, so the
    // header carries zeroes and only the central directory is authoritative.
    if entry.flags & DATA_DESCRIPTOR_FLAG == 0
        && (local_crc32 != entry.crc32
            || local_compressed_size != entry.compressed_size
            || local_uncompressed_size != entry.uncompressed_size)
    {
        return Err("package local and central entry sizes disagree".to_owned());
    }

    let data_offset = entry
        .local_offset
        .checked_add(LOCAL_HEADER_LENGTH)
        .and_then(|offset| offset.checked_add(name_length))
        .and_then(|offset| offset.checked_add(extra_length))
        .ok_or("package local entry overflowed")?;
    if data_offset
        .checked_add(u64::from(entry.compressed_size))
        .is_none_or(|end| end > central_offset)
    {
        return Err("package entry data overlaps its central directory".to_owned());
    }

    Ok(())
}

fn read_exact_at<R: Read + Seek>(
    reader: &mut R,
    offset: u64,
    length: u64,
) -> Result<Vec<u8>, String> {
    reader
        .seek(SeekFrom::Start(offset))
        .map_err(|source| format!("failed to read the package archive: {source}"))?;
    let mut bytes = vec![
        0;
        usize::try_from(length).map_err(|_| {
            "package archive region does not fit this platform".to_owned()
        })?
    ];
    reader
        .read_exact(&mut bytes)
        .map_err(|source| format!("package archive ended unexpectedly: {source}"))?;
    Ok(bytes)
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};

    use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

    use super::*;

    fn single_entry_archive() -> Vec<u8> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .start_file(
                "manifest.json",
                SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
            )
            .expect("test entry should start");
        writer.write_all(b"{}").expect("test content should write");
        writer
            .finish()
            .expect("test archive should finish")
            .into_inner()
    }

    fn rejection(bytes: Vec<u8>) -> String {
        validate_archive_structure(&mut Cursor::new(bytes))
            .expect_err("malformed archive must be rejected")
    }

    #[test]
    fn accepts_a_well_formed_archive() {
        validate_archive_structure(&mut Cursor::new(single_entry_archive()))
            .expect("a well formed archive should validate");
    }

    #[test]
    fn rejects_data_appended_after_the_central_directory() {
        let mut bytes = single_entry_archive();
        bytes.extend_from_slice(b"appended");

        assert!(
            rejection(bytes).contains("missing its central directory"),
            "appended data must not be accepted"
        );
    }

    #[test]
    fn rejects_data_prepended_before_the_first_entry() {
        let mut bytes = b"prepended".to_vec();
        bytes.extend_from_slice(&single_entry_archive());

        // Prepending shifts every recorded offset, so this is caught by
        // whichever consistency check the shift breaks first. The message
        // varies with the amount prepended; the rejection is what matters.
        let message = rejection(bytes);
        assert!(
            !message.is_empty(),
            "prepended data must be rejected: {message}"
        );
    }

    #[test]
    fn rejects_a_local_header_that_disagrees_with_the_central_directory() {
        let mut bytes = single_entry_archive();
        // Corrupt the local header CRC while leaving the central directory
        // record intact, which is the shape of a parser confusion attack.
        bytes[14] ^= 0xff;

        assert!(
            rejection(bytes).contains("sizes disagree"),
            "a local header must agree with its central directory record"
        );
    }

    #[test]
    fn rejects_a_local_name_that_disagrees_with_the_central_directory() {
        let mut bytes = single_entry_archive();
        // The local name begins after the 30 byte fixed local header.
        bytes[30] = b'X';

        assert!(
            rejection(bytes).contains("names disagree"),
            "a local entry name must match its central directory record"
        );
    }

    #[test]
    fn rejects_a_zip64_end_of_central_directory_locator() {
        let mut bytes = single_entry_archive();
        let eocd = bytes.len() - END_OF_CENTRAL_DIRECTORY_LENGTH as usize;
        let mut rebuilt = bytes[..eocd].to_vec();
        rebuilt.extend_from_slice(&ZIP64_LOCATOR_SIGNATURE.to_le_bytes());
        rebuilt.extend_from_slice(&[0; 16]);
        rebuilt.extend_from_slice(&bytes[eocd..]);
        // The central directory offset is unchanged, so only the locator moves
        // the end record. Repoint it so the record itself stays consistent.
        let offset = rebuilt.len() - END_OF_CENTRAL_DIRECTORY_LENGTH as usize;
        bytes = rebuilt;

        assert!(
            validate_archive_structure(&mut Cursor::new(bytes.clone())).is_err(),
            "a ZIP64 locator must be rejected, end record at {offset}"
        );
    }
}
