//! Synchronous, policy-free HID transport for Bosun.
//!
//! This crate moves HID reports and nothing else. It is deliberately blocking
//! (async bridging belongs to the M4 daemon) and holds no product knowledge:
//! a device is a set of match criteria plus, above this layer, a descriptor.
//!
//! The only backend is the stock OS HID stack via `hidapi`, per ADR-0001.
//!
//! # Testing without hardware
//!
//! [`MockTransport`] replays a scripted sequence of reports, timeouts, and
//! disconnects, so every layer above this crate is testable with nothing
//! plugged in.
//!
//! ```
//! use std::time::Duration;
//! use bosun_hid::{DeviceInfo, MockTransport, ReadOutcome, Transport};
//!
//! let mut transport = MockTransport::new(DeviceInfo::default())
//!     .push_timeout()
//!     .push_report(&[0x01, 0x80, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00]);
//!
//! let mut buf = [0u8; 8];
//! assert_eq!(
//!     transport.read(&mut buf, Duration::from_millis(10)).unwrap(),
//!     ReadOutcome::Timeout
//! );
//! assert_eq!(
//!     transport.read(&mut buf, Duration::from_millis(10)).unwrap(),
//!     ReadOutcome::Report(8)
//! );
//! ```

pub mod device;
pub mod error;
pub mod hid;
pub mod mock;
pub mod transport;

pub use device::{select, select_all, select_index, DeviceInfo, DeviceMatch};
pub use error::{HidError, Result};
pub use hid::HidTransport;
pub use mock::{MockTransport, ScriptedRead};
pub use transport::{ReadOutcome, Transport};
