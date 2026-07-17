use std::io::{self, Read, Write};

#[derive(Debug, thiserror::Error)]
pub(crate) enum BoundedIoError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("content exceeds the configured limit of {limit} bytes")]
    LimitExceeded { limit: u64 },
}

pub(crate) fn read_to_end(reader: &mut impl Read, limit: u64) -> Result<Vec<u8>, BoundedIoError> {
    let capacity = usize::try_from(limit.min(64 * 1024)).unwrap_or(64 * 1024);
    let mut output = Vec::with_capacity(capacity);
    copy(reader, &mut output, limit)?;
    Ok(output)
}

pub(crate) fn copy(
    reader: &mut impl Read,
    writer: &mut impl Write,
    limit: u64,
) -> Result<u64, BoundedIoError> {
    let mut total = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            return Ok(total);
        }
        let count = u64::try_from(count).expect("read buffer length fits in u64");
        if total.saturating_add(count) > limit {
            return Err(BoundedIoError::LimitExceeded { limit });
        }
        writer.write_all(&buffer[..usize::try_from(count).expect("buffer count fits in usize")])?;
        total += count;
    }
}
