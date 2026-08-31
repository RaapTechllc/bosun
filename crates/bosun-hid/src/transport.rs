//! The synchronous transport abstraction.
//!
//! `Transport` is deliberately blocking and free of policy: it moves reports,
//! and nothing else. Async bridging is a daemon concern (M4), not this crate's.

use std::time::Duration;

use crate::device::DeviceInfo;
use crate::error::Result;

/// The result of one read attempt.
///
/// Timeouts are a distinct outcome rather than a zero-length report, so a
/// caller polling for input cannot silently confuse "nothing happened" with
/// "the device went away".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadOutcome {
    /// A report was written into the caller's buffer, this many bytes long.
    Report(usize),
    /// The timeout elapsed with no report available.
    Timeout,
}

/// A synchronous, policy-free HID channel to one opened interface.
pub trait Transport {
    /// Metadata for the interface this transport is bound to.
    fn info(&self) -> &DeviceInfo;

    /// Read one input report, waiting at most `timeout`.
    fn read(&mut self, buf: &mut [u8], timeout: Duration) -> Result<ReadOutcome>;

    /// Write one output report. `data[0]` is the report ID.
    fn write(&mut self, data: &[u8]) -> Result<usize>;

    /// Send one feature report. `data[0]` is the report ID.
    fn send_feature_report(&mut self, data: &[u8]) -> Result<()>;

    /// Read one feature report. `buf[0]` must hold the requested report ID.
    fn get_feature_report(&mut self, buf: &mut [u8]) -> Result<usize>;
}

/// Clamp a timeout to the non-negative millisecond range the HID backend takes.
///
/// A negative value means "block forever" to `hidapi`, which would turn a long
/// timeout into a hang, so saturate instead of wrapping.
pub(crate) fn timeout_millis(timeout: Duration) -> i32 {
    i32::try_from(timeout.as_millis()).unwrap_or(i32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_timeout_polls_without_blocking() {
        assert_eq!(timeout_millis(Duration::ZERO), 0);
    }

    #[test]
    fn ordinary_timeouts_convert_exactly() {
        assert_eq!(timeout_millis(Duration::from_millis(1)), 1);
        assert_eq!(timeout_millis(Duration::from_secs(1)), 1_000);
        assert_eq!(timeout_millis(Duration::from_secs(2)), 2_000);
    }

    #[test]
    fn oversized_timeouts_saturate_instead_of_going_negative() {
        // The failure this guards against: a wrapped conversion yielding a
        // negative value, which `hidapi` reads as "block forever".
        assert_eq!(timeout_millis(Duration::MAX), i32::MAX);
        assert_eq!(
            timeout_millis(Duration::from_millis(i32::MAX as u64 + 1)),
            i32::MAX
        );
    }

    #[test]
    fn no_duration_ever_converts_to_a_blocking_read() {
        let durations = [
            Duration::ZERO,
            Duration::from_millis(1),
            Duration::from_secs(2),
            Duration::from_secs(u32::MAX as u64),
            Duration::MAX,
        ];

        for duration in durations {
            assert!(
                timeout_millis(duration) >= 0,
                "{duration:?} produced a blocking timeout"
            );
        }
    }
}
