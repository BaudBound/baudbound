use rusqlite::types::Type;

use crate::{StorageError, storage::filesystem::current_unix_timestamp};

pub(super) fn unix_timestamp_for_sqlite() -> Result<i64, StorageError> {
    i64::try_from(current_unix_timestamp()).map_err(|_| {
        StorageError::Operation("current Unix timestamp is too large for SQLite".to_owned())
    })
}

pub(super) fn bool_to_sqlite(value: bool) -> i64 {
    i64::from(value)
}

pub(super) fn u32_to_sqlite(value: u32) -> i64 {
    i64::from(value)
}

pub(super) fn u64_to_sqlite(value: u64) -> Result<i64, StorageError> {
    i64::try_from(value)
        .map_err(|_| StorageError::Operation(format!("{value} is too large for SQLite")))
}

pub(super) fn usize_to_sqlite(value: usize) -> Result<i64, StorageError> {
    i64::try_from(value)
        .map_err(|_| StorageError::Operation(format!("{value} is too large for SQLite")))
}

pub(super) fn row_i64_to_bool(index: usize, value: i64) -> rusqlite::Result<bool> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(row_integer_conversion_error(
            index,
            format!("{value} is not a valid boolean"),
        )),
    }
}

pub(super) fn row_i64_to_u32(index: usize, value: i64) -> rusqlite::Result<u32> {
    u32::try_from(value).map_err(|_| {
        row_integer_conversion_error(index, format!("{value} is outside the u32 range"))
    })
}

pub(super) fn row_i64_to_u64(index: usize, value: i64) -> rusqlite::Result<u64> {
    u64::try_from(value).map_err(|_| {
        row_integer_conversion_error(index, format!("{value} is outside the u64 range"))
    })
}

pub(super) fn row_i64_to_usize(index: usize, value: i64) -> rusqlite::Result<usize> {
    usize::try_from(value).map_err(|_| {
        row_integer_conversion_error(index, format!("{value} is outside the usize range"))
    })
}

fn row_integer_conversion_error(index: usize, message: String) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        index,
        Type::Integer,
        Box::new(StorageError::Operation(message)),
    )
}
