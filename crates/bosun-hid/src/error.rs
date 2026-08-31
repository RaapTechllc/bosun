//! Backend-agnostic errors.
//!
//! Nothing above `bosun-hid` should have to know which HID backend produced a
//! failure, so backend errors are flattened into [`HidError::Backend`].

use crate::device::DeviceMatch;

pub type Result<T> = std::result::Result<T, HidError>;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum HidError {
    /// Enumeration completed but no interface satisfied the criteria.
    #[error("no HID interface matched {0}")]
    NotFound(DeviceMatch),

    /// The device went away. Callers that want hot-plug recovery re-enumerate
    /// and re-open rather than retrying on the dead handle.
    #[error("the device is no longer connected")]
    Disconnected,

    /// The caller's buffer cannot hold the report.
    #[error("buffer holds {actual} bytes but the report needs {expected}")]
    BufferTooSmall { expected: usize, actual: usize },

    /// Any failure reported by the underlying HID backend.
    #[error("HID backend failure: {0}")]
    Backend(String),
}
