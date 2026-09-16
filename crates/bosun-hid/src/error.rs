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

/// A TOML descriptor that could not be loaded or validated.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum DescriptorError {
    /// The file could not be read.
    #[error("failed to read device descriptor {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },

    /// The document is not a valid descriptor. Never a panic.
    #[error("malformed device descriptor: {0}")]
    Malformed(String),
}

/// An input report or descriptor that the codec cannot decode.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CodecError {
    /// Input reports are 8 bytes including the report ID.
    #[error("input report must be 8 bytes, got {0}")]
    WrongLength(usize),

    /// Vendor input is report ID `0x01`.
    #[error("input report ID must be 0x01, got 0x{0:02x}")]
    WrongReportId(u8),

    /// The descriptor cannot drive a 40-bit key field.
    #[error("device descriptor cannot drive the codec: {0}")]
    Descriptor(String),

    /// A recorded fixture line is not eight hex bytes.
    #[error("recorded report is not 8 hex bytes: {0}")]
    InvalidRecording(String),
}
