use std::io::{self, Read, Write};

use baudbound_runtime::ResourceLimit;

#[derive(Debug, thiserror::Error)]
pub(crate) enum BoundedIoError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("content exceeds the configured limit of {limit} bytes")]
    LimitExceeded { limit: ResourceLimit },
}

pub(crate) fn read_to_end(
    reader: &mut impl Read,
    limit: ResourceLimit,
) -> Result<Vec<u8>, BoundedIoError> {
    let capacity = limit.value().map_or(64 * 1024, |value| {
        usize::try_from(value.min(64 * 1024)).unwrap_or(64 * 1024)
    });
    let mut output = Vec::with_capacity(capacity);
    copy(reader, &mut output, limit)?;
    Ok(output)
}

pub(crate) fn copy(
    reader: &mut impl Read,
    writer: &mut impl Write,
    limit: ResourceLimit,
) -> Result<u64, BoundedIoError> {
    let mut total = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            return Ok(total);
        }
        let count = u64::try_from(count).expect("read buffer length fits in u64");
        if limit.is_exceeded_by(total.saturating_add(count)) {
            return Err(BoundedIoError::LimitExceeded { limit });
        }
        writer.write_all(&buffer[..usize::try_from(count).expect("buffer count fits in usize")])?;
        total += count;
    }
}
