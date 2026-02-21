use std::time::{SystemTime, UNIX_EPOCH};

pub fn now_nanos() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}

pub fn to_i64_u64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or_else(|_| {
        panic!("invariant violated: value {value} does not fit signed 64-bit SQLite integer")
    })
}
