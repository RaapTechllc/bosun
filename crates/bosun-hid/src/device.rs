//! Device identity, enumeration metadata, and selection criteria.
//!
//! This module is policy-free: it knows nothing about any particular product.
//! Callers supply match criteria; the G13 lives in device data, not here.

use std::fmt;

/// Metadata for one enumerated HID interface.
///
/// Windows enumerates a separate entry per top-level collection, so a single
/// physical device can appear several times under one VID/PID with different
/// usage pages. `hidraw` and `IOHIDManager` do the same for composite devices.
/// Selection therefore has to consider the usage page, never VID/PID alone.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeviceInfo {
    /// Backend-specific path that opens this exact interface.
    pub path: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub usage_page: u16,
    pub usage: u16,
    pub interface_number: i32,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    pub serial_number: Option<String>,
}

/// Criteria that identify one HID interface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeviceMatch {
    pub vendor_id: u16,
    pub product_id: u16,
    pub usage_page: u16,
}

impl DeviceMatch {
    pub const fn new(vendor_id: u16, product_id: u16, usage_page: u16) -> Self {
        Self {
            vendor_id,
            product_id,
            usage_page,
        }
    }

    /// True when `info` satisfies every criterion.
    pub fn matches(&self, info: &DeviceInfo) -> bool {
        self.vendor_id == info.vendor_id
            && self.product_id == info.product_id
            && self.usage_page == info.usage_page
    }
}

impl fmt::Display for DeviceMatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:04x}:{:04x} usage_page={:04x}",
            self.vendor_id, self.product_id, self.usage_page
        )
    }
}

/// Index of the first interface in enumeration order satisfying `criteria`.
///
/// Backends hand back their own richer handle alongside this metadata, so
/// callers that need to reach it match on the index rather than on the path.
pub fn select_index(devices: &[DeviceInfo], criteria: &DeviceMatch) -> Option<usize> {
    devices.iter().position(|info| criteria.matches(info))
}

/// The first interface in enumeration order that satisfies `criteria`.
///
/// Enumeration order is the backend's own, so this is stable for a given
/// machine and cabling. Open the returned [`DeviceInfo::path`] rather than
/// re-deriving a path from VID/PID.
pub fn select<'a>(devices: &'a [DeviceInfo], criteria: &DeviceMatch) -> Option<&'a DeviceInfo> {
    select_index(devices, criteria).map(|index| &devices[index])
}

/// Every interface satisfying `criteria`, in enumeration order.
pub fn select_all<'a>(devices: &'a [DeviceInfo], criteria: &DeviceMatch) -> Vec<&'a DeviceInfo> {
    devices
        .iter()
        .filter(|info| criteria.matches(info))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Criteria for the interface M1 targets, used only as test data.
    const TARGET: DeviceMatch = DeviceMatch::new(0x046D, 0xC21C, 0xFF00);

    fn interface(path: &str, vendor_id: u16, product_id: u16, usage_page: u16) -> DeviceInfo {
        DeviceInfo {
            path: path.to_owned(),
            vendor_id,
            product_id,
            usage_page,
            ..DeviceInfo::default()
        }
    }

    #[test]
    fn matches_requires_all_three_fields() {
        let target = interface("a", 0x046D, 0xC21C, 0xFF00);
        assert!(TARGET.matches(&target));

        assert!(!TARGET.matches(&interface("a", 0x046E, 0xC21C, 0xFF00)));
        assert!(!TARGET.matches(&interface("a", 0x046D, 0xC21D, 0xFF00)));
    }

    #[test]
    fn matching_vid_pid_with_a_different_usage_page_is_rejected() {
        // The keyboard collection of the same physical device. Opening it
        // yields no vendor reports, so VID/PID alone is not a valid match.
        let keyboard_collection = interface("kbd", 0x046D, 0xC21C, 0x0001);

        assert!(!TARGET.matches(&keyboard_collection));
    }

    #[test]
    fn select_picks_the_vendor_collection_from_a_composite_device() {
        let devices = vec![
            interface("kbd", 0x046D, 0xC21C, 0x0001),
            interface("consumer", 0x046D, 0xC21C, 0x000C),
            interface("vendor", 0x046D, 0xC21C, 0xFF00),
        ];

        let chosen = select(&devices, &TARGET).expect("vendor collection is present");

        assert_eq!(chosen.path, "vendor");
    }

    #[test]
    fn select_returns_none_when_nothing_matches() {
        let devices = vec![interface("kbd", 0x046D, 0xC21C, 0x0001)];

        assert!(select(&devices, &TARGET).is_none());
        assert!(select(&[], &TARGET).is_none());
    }

    #[test]
    fn select_is_deterministic_when_several_interfaces_match() {
        let devices = vec![
            interface("first", 0x046D, 0xC21C, 0xFF00),
            interface("second", 0x046D, 0xC21C, 0xFF00),
        ];

        let chosen = select(&devices, &TARGET).expect("a match is present");

        assert_eq!(chosen.path, "first");
    }

    #[test]
    fn select_index_points_at_the_matched_interface() {
        let devices = vec![
            interface("kbd", 0x046D, 0xC21C, 0x0001),
            interface("consumer", 0x046D, 0xC21C, 0x000C),
            interface("vendor", 0x046D, 0xC21C, 0xFF00),
        ];

        // The index is what lets a backend recover its own handle for the
        // interface this crate chose.
        assert_eq!(select_index(&devices, &TARGET), Some(2));
        assert_eq!(select_index(&[], &TARGET), None);
    }

    #[test]
    fn select_all_returns_every_match_in_enumeration_order() {
        let devices = vec![
            interface("first", 0x046D, 0xC21C, 0xFF00),
            interface("kbd", 0x046D, 0xC21C, 0x0001),
            interface("second", 0x046D, 0xC21C, 0xFF00),
        ];

        let paths: Vec<&str> = select_all(&devices, &TARGET)
            .iter()
            .map(|info| info.path.as_str())
            .collect();

        assert_eq!(paths, ["first", "second"]);
    }

    #[test]
    fn display_is_stable_for_error_messages() {
        assert_eq!(TARGET.to_string(), "046d:c21c usage_page=ff00");
    }
}
