use crate::argconv::*;
use crate::cass_error::CassError;
use crate::types::*;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;
use std::os::raw::c_char;
use std::process;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

pub use crate::cass_uuid_types::CassUuid;

pub struct CassUuidGen {
    pub(crate) clock_seq_and_node: cass_uint64_t,
    pub(crate) last_timestamp: AtomicU64,
}

impl FFI for CassUuidGen {
    type Origin = FromBox;
}

// Implementation directly ported from Cpp Driver implementation:

const TIME_OFFSET_BETWEEN_UTC_AND_EPOCH: u64 = 0x01B21DD213814000; // Nanoseconds
const MIN_CLOCK_SEQ_AND_NODE: u64 = 0x8080808080808080;
const MAX_CLOCK_SEQ_AND_NODE: u64 = 0x7f7f7f7f7f7f7f7f;

fn to_milliseconds(timestamp: u64) -> u64 {
    timestamp / 10000
}

fn from_unix_timestamp(timestamp: u64) -> u64 {
    (timestamp.wrapping_mul(10000)).wrapping_add(TIME_OFFSET_BETWEEN_UTC_AND_EPOCH)
}

fn set_version(timestamp: u64, version: u8) -> u64 {
    (timestamp & 0x0FFFFFFFFFFFFFFF) | ((version as u64) << 60)
}

// Ported from UuidGen::set_clock_seq_and_node
fn rand_clock_seq_and_node(node: u64) -> u64 {
    let clock_seq: u64 = rand::random();
    let mut result: u64 = 0;
    result |= (clock_seq & 0x0000000000003FFF) << 48;
    result |= 0x8000000000000000; // RFC4122 variant
    result |= node;
    result
}

fn try_monotonic_timestamp(last_timestamp: &AtomicU64, now: u64) -> Option<u64> {
    let last = last_timestamp.load(Ordering::SeqCst);

    // The wall clock advanced, so use its current millisecond as the new baseline.
    if now > last {
        return last_timestamp
            .compare_exchange(last, now, Ordering::SeqCst, Ordering::SeqCst)
            .map(|_| now)
            .ok();
    }

    let last_ms = to_milliseconds(last);
    // Preserve monotonicity after clock rollback or when another thread advanced the timestamp
    // after this thread sampled the clock.
    if to_milliseconds(now) < last_ms {
        return Some(
            last_timestamp
                .fetch_add(1, Ordering::SeqCst)
                .wrapping_add(1),
        );
    }

    // Allocate the next 100-nanosecond tick within the current wall-clock millisecond.
    let candidate = last.wrapping_add(1);
    if to_milliseconds(candidate) == last_ms {
        return last_timestamp
            .compare_exchange(last, candidate, Ordering::SeqCst, Ordering::SeqCst)
            .map(|_| candidate)
            .ok();
    }

    None
}

// Ported from UuidGen::monotonic_timestamp.
fn monotonic_timestamp(last_timestamp: &AtomicU64) -> u64 {
    loop {
        let now = SystemTime::now();
        let now = now.duration_since(UNIX_EPOCH).unwrap();
        let now = from_unix_timestamp(now.as_millis() as u64);

        if let Some(timestamp) = try_monotonic_timestamp(last_timestamp, now) {
            return timestamp;
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn cass_uuid_version(uuid: CassUuid) -> cass_uint8_t {
    ((uuid.time_and_version >> 60) & 0x0F) as cass_uint8_t
}

#[unsafe(no_mangle)]
pub extern "C" fn cass_uuid_timestamp(uuid: CassUuid) -> cass_uint64_t {
    let timestamp: u64 = uuid.time_and_version & 0x0FFFFFFFFFFFFFFF;
    to_milliseconds(timestamp.wrapping_sub(TIME_OFFSET_BETWEEN_UTC_AND_EPOCH))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_uuid_min_from_time(timestamp: cass_uint64_t, output: *mut CassUuid) {
    let uuid = CassUuid {
        time_and_version: set_version(from_unix_timestamp(timestamp), 1),
        clock_seq_and_node: MIN_CLOCK_SEQ_AND_NODE,
    };

    unsafe { std::ptr::write(output, uuid) };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_uuid_max_from_time(timestamp: cass_uint64_t, output: *mut CassUuid) {
    let uuid = CassUuid {
        time_and_version: set_version(from_unix_timestamp(timestamp), 1),
        clock_seq_and_node: MAX_CLOCK_SEQ_AND_NODE,
    };

    unsafe { std::ptr::write(output, uuid) };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_uuid_gen_new() -> CassOwnedExclusivePtr<CassUuidGen, CMut> {
    // Inspired by C++ driver implementation in its intent.
    // The original driver tries to generate a number that
    // uniquely identifies this machine and the current process.

    // In the original driver, it generates a number
    // based on local IPs, CPU info and PID.
    let machine_id = machine_uid::get().unwrap();
    let pid = process::id();

    let mut hasher = DefaultHasher::new();
    machine_id.hash(&mut hasher);
    pid.hash(&mut hasher);

    // Masking the same way as in Cpp Driver.
    let node: u64 = (hasher.finish() & 0x0000FFFFFFFFFFFF) | 0x0000010000000000 /* Multicast bit */;

    BoxFFI::into_ptr(Box::new(CassUuidGen {
        clock_seq_and_node: rand_clock_seq_and_node(node),
        last_timestamp: AtomicU64::new(0),
    }))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_uuid_gen_new_with_node(
    node: cass_uint64_t,
) -> CassOwnedExclusivePtr<CassUuidGen, CMut> {
    BoxFFI::into_ptr(Box::new(CassUuidGen {
        clock_seq_and_node: rand_clock_seq_and_node(node & 0x0000FFFFFFFFFFFF),
        last_timestamp: AtomicU64::new(0),
    }))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_uuid_gen_time(
    uuid_gen: CassBorrowedSharedPtr<CassUuidGen, CMut>,
    output: *mut CassUuid,
) {
    let Some(uuid_gen) = BoxFFI::as_ref(uuid_gen) else {
        tracing::error!("Provided null uuid generator pointer to cass_uuid_gen_time!");
        return;
    };

    let uuid = CassUuid {
        time_and_version: set_version(monotonic_timestamp(&uuid_gen.last_timestamp), 1),
        clock_seq_and_node: uuid_gen.clock_seq_and_node,
    };

    unsafe { std::ptr::write(output, uuid) };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_uuid_gen_random(_uuid_gen: *mut CassUuidGen, output: *mut CassUuid) {
    let time_and_version: u64 = rand::random();
    let clock_seq_and_node: u64 = rand::random();

    // RFC4122 variant
    let uuid = CassUuid {
        time_and_version: set_version(time_and_version, 4),
        clock_seq_and_node: (clock_seq_and_node & 0x3FFFFFFFFFFFFFFF) | 0x8000000000000000,
    };

    unsafe { std::ptr::write(output, uuid) };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_uuid_gen_from_time(
    uuid_gen: CassBorrowedSharedPtr<CassUuidGen, CMut>,
    timestamp: cass_uint64_t,
    output: *mut CassUuid,
) {
    let Some(uuid_gen) = BoxFFI::as_ref(uuid_gen) else {
        tracing::error!("Provided null uuid generator pointer to cass_uuid_gen_from_time!");
        return;
    };

    let uuid = CassUuid {
        time_and_version: set_version(from_unix_timestamp(timestamp), 1),
        clock_seq_and_node: uuid_gen.clock_seq_and_node,
    };

    unsafe { std::ptr::write(output, uuid) };
}

// Implemented ourselves:

impl From<CassUuid> for Uuid {
    fn from(uuid: CassUuid) -> Self {
        // This is a strange representation that Cpp driver
        // employs. "Recovered" and validated it empirically...
        let time_and_version_msb = uuid.time_and_version & 0xFFFFFFFF;
        let time_and_version_lsb =
            (((uuid.time_and_version & 0xFFFFFFFF00000000) >> 32) as u32).rotate_left(16) as u64;

        let msb = ((time_and_version_msb << 32) | time_and_version_lsb) as u128;
        let lsb = uuid.clock_seq_and_node as u128;

        Uuid::from_u128((msb << 64) | lsb)
    }
}

impl From<Uuid> for CassUuid {
    fn from(uuid: Uuid) -> Self {
        let u128_representation = uuid.as_u128();
        let upper_u64 = (u128_representation >> 64) as u64;
        let lower_u64 = (u128_representation & 0xFFFFFFFFFFFFFFFF) as u64;

        // This is a strange representation that Cpp driver
        // employs. "Recovered" and validated it empirically...
        let msb = ((upper_u64 & 0xFFFFFFFF) as u32).rotate_left(16) as u64;
        let lsb = (upper_u64 & 0xFFFFFFFF00000000) >> 32;

        CassUuid {
            time_and_version: (msb << 32) | lsb,
            clock_seq_and_node: lower_u64,
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_uuid_string(uuid_raw: CassUuid, output: *mut c_char) {
    let uuid: Uuid = uuid_raw.into();

    let string_representation = uuid.hyphenated().to_string();
    unsafe {
        std::ptr::copy_nonoverlapping(
            string_representation.as_ptr(),
            output as *mut u8,
            string_representation.len(),
        );

        // Null-terminate
        let null_byte = output.add(string_representation.len()) as *mut c_char;
        *null_byte = 0;
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_uuid_from_string(
    value: CassStrNulTerminated<'_>,
    output: *mut CassUuid,
) -> CassError {
    let (value, value_length) = unsafe { value.as_len_delimited() };
    unsafe { cass_uuid_from_string_n(value, value_length, output) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_uuid_from_string_n(
    value: CassStrLenDelimited<'_>,
    value_length: CassStrLen,
    output: *mut CassUuid,
) -> CassError {
    let value_str = match unsafe { value.to_str(value_length) } {
        Ok(s) => s,
        Err(PtrToStrError::NullPointer) => {
            tracing::error!("Provided null string pointer to cass_uuid_from_string(_n)!");
            return CassError::CASS_ERROR_LIB_BAD_PARAMS;
        }
        Err(PtrToStrError::InvalidUtf8(_)) => {
            tracing::error!("Provided non-UTF8 string to cass_uuid_from_string(_n)!");
            return CassError::CASS_ERROR_LIB_BAD_PARAMS;
        }
    };
    Uuid::parse_str(value_str).map_or(CassError::CASS_ERROR_LIB_BAD_PARAMS, |parsed_uuid| {
        unsafe { std::ptr::write(output, parsed_uuid.into()) };
        CassError::CASS_OK
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cass_uuid_gen_free(uuid_gen: CassOwnedExclusivePtr<CassUuidGen, CMut>) {
    BoxFFI::free(uuid_gen);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};
    use std::thread;

    #[test]
    fn monotonic_timestamp_uses_submillisecond_ticks() {
        let now = from_unix_timestamp(1_700_000_000_000);
        let last_timestamp = AtomicU64::new(now);

        for expected_offset in 1..10_000 {
            assert_eq!(
                try_monotonic_timestamp(&last_timestamp, now),
                Some(now + expected_offset)
            );
        }

        assert_eq!(try_monotonic_timestamp(&last_timestamp, now), None);
    }

    #[test]
    fn monotonic_timestamp_remains_monotonic_during_clock_rollback() {
        let now = from_unix_timestamp(1_700_000_000_000);
        let future = from_unix_timestamp(1_700_000_000_001);
        let last_timestamp = AtomicU64::new(future);

        assert_eq!(
            try_monotonic_timestamp(&last_timestamp, now),
            Some(future + 1)
        );
        assert_eq!(last_timestamp.load(Ordering::SeqCst), future + 1);
    }

    #[test]
    fn monotonic_timestamp_handles_stale_sample_across_millisecond_boundary() {
        let stale_now = from_unix_timestamp(1_700_000_000_000);
        let next_millisecond = from_unix_timestamp(1_700_000_000_001);
        let last_timestamp = AtomicU64::new(stale_now);

        assert_eq!(
            try_monotonic_timestamp(&last_timestamp, next_millisecond),
            Some(next_millisecond)
        );
        assert_eq!(
            try_monotonic_timestamp(&last_timestamp, stale_now),
            Some(next_millisecond + 1)
        );
    }

    #[test]
    fn monotonic_timestamp_is_unique_under_concurrency() {
        const THREADS: usize = 8;
        const UUIDS_PER_THREAD: usize = 1_000;

        let now = from_unix_timestamp(1_700_000_000_000);
        let last_timestamp = Arc::new(AtomicU64::new(0));
        let start = Arc::new(Barrier::new(THREADS));
        let mut handles = Vec::with_capacity(THREADS);

        for _ in 0..THREADS {
            let last_timestamp = Arc::clone(&last_timestamp);
            let start = Arc::clone(&start);
            handles.push(thread::spawn(move || {
                let mut timestamps = Vec::with_capacity(UUIDS_PER_THREAD);
                start.wait();
                for _ in 0..UUIDS_PER_THREAD {
                    loop {
                        if let Some(timestamp) = try_monotonic_timestamp(&last_timestamp, now) {
                            timestamps.push(timestamp);
                            break;
                        }
                    }
                }
                timestamps
            }));
        }

        let mut timestamps: Vec<_> = handles
            .into_iter()
            .flat_map(|handle| handle.join().unwrap())
            .collect();
        timestamps.sort_unstable();
        timestamps.dedup();

        assert_eq!(timestamps.len(), THREADS * UUIDS_PER_THREAD);
        assert_eq!(timestamps[0], now);
        assert_eq!(timestamps[timestamps.len() - 1], now + 7_999);
    }
}
