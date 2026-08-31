//! The `hidapi` transport over the stock OS HID stack.
//!
//! Per ADR-0001 this is the only backend: Windows HID API, macOS
//! `IOHIDManager`, and Linux `hidraw`. No libusb, WinUSB, kext, or kernel
//! module is used or supported.

use std::ffi::CString;
use std::time::Duration;

use hidapi::{DeviceInfo as HidDeviceInfo, HidApi, HidDevice};

use crate::device::{self, DeviceInfo, DeviceMatch};
use crate::error::{HidError, Result};
use crate::transport::{timeout_millis, ReadOutcome, Transport};

/// A blocking [`Transport`] bound to one opened HID interface.
pub struct HidTransport {
    device: HidDevice,
    info: DeviceInfo,
}

impl HidTransport {
    /// Create the backend handle used for enumeration and opening.
    ///
    /// `hidapi` permits only one live [`HidApi`] per process, so hold this for
    /// the lifetime of the process rather than creating one per operation.
    /// Re-enumerate after a hot-plug with [`HidApi::refresh_devices`].
    pub fn api() -> Result<HidApi> {
        HidApi::new().map_err(backend_error)
    }

    /// Every HID interface the OS currently exposes, in enumeration order.
    pub fn enumerate(api: &HidApi) -> Vec<DeviceInfo> {
        api.device_list().map(convert).collect()
    }

    /// Match on VID, PID, and usage page, then open the enumerated path of the
    /// interface that matched.
    ///
    /// Matching on VID/PID alone would open whichever collection the OS
    /// happened to list first, which for a composite device is usually the
    /// wrong one.
    pub fn open(api: &HidApi, criteria: &DeviceMatch) -> Result<Self> {
        let entries: Vec<&HidDeviceInfo> = api.device_list().collect();
        let mut infos: Vec<DeviceInfo> = entries.iter().copied().map(convert).collect();

        let index = match device::select_index(&infos, criteria) {
            Some(index) => index,
            None => return Err(HidError::NotFound(*criteria)),
        };
        let device = api
            .open_path(entries[index].path())
            .map_err(backend_error)?;

        Ok(Self {
            info: infos.swap_remove(index),
            device,
        })
    }

    /// Open one specific enumerated path.
    pub fn open_path(api: &HidApi, path: &str) -> Result<Self> {
        let info = Self::enumerate(api)
            .into_iter()
            .find(|info| info.path == path)
            .ok_or_else(|| HidError::Backend(format!("no enumerated HID interface at {path}")))?;
        let c_path = CString::new(path)
            .map_err(|_| HidError::Backend(format!("HID path contains an interior NUL: {path}")))?;
        let device = api.open_path(&c_path).map_err(backend_error)?;

        Ok(Self { device, info })
    }
}

impl Transport for HidTransport {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    /// Read one input report.
    ///
    /// The backend cannot distinguish an unplug from other failures portably,
    /// so a caller wanting hot-plug recovery treats any error here as a signal
    /// to re-enumerate and re-open rather than to retry this handle.
    fn read(&mut self, buf: &mut [u8], timeout: Duration) -> Result<ReadOutcome> {
        match self.device.read_timeout(buf, timeout_millis(timeout)) {
            Ok(0) => Ok(ReadOutcome::Timeout),
            Ok(len) => Ok(ReadOutcome::Report(len)),
            Err(error) => Err(backend_error(error)),
        }
    }

    fn write(&mut self, data: &[u8]) -> Result<usize> {
        self.device.write(data).map_err(backend_error)
    }

    fn send_feature_report(&mut self, data: &[u8]) -> Result<()> {
        self.device.send_feature_report(data).map_err(backend_error)
    }

    fn get_feature_report(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.device.get_feature_report(buf).map_err(backend_error)
    }
}

/// Flatten a backend error so callers never depend on the `hidapi` types.
fn backend_error(error: hidapi::HidError) -> HidError {
    HidError::Backend(error.to_string())
}

fn convert(entry: &HidDeviceInfo) -> DeviceInfo {
    DeviceInfo {
        // Paths are ASCII device paths on every supported platform; the lossy
        // conversion exists so the type above this crate stays a plain String.
        path: entry.path().to_string_lossy().into_owned(),
        vendor_id: entry.vendor_id(),
        product_id: entry.product_id(),
        usage_page: entry.usage_page(),
        usage: entry.usage(),
        interface_number: entry.interface_number(),
        manufacturer: entry.manufacturer_string().map(str::to_owned),
        product: entry.product_string().map(str::to_owned),
        serial_number: entry.serial_number().map(str::to_owned),
    }
}
