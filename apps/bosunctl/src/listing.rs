//! Argument parsing and rendering for `bosunctl device list`.
//!
//! Kept free of `clap` and of any HID backend so it can be unit-tested on its
//! own.

use bosun_hid::DeviceInfo;

/// Parse a 16-bit identifier written in decimal or `0x`-prefixed hex.
pub fn parse_u16(raw: &str) -> Result<u16, String> {
    let text = raw.trim();
    let parsed = match text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        Some(hex) => u16::from_str_radix(hex, 16),
        None => text.parse::<u16>(),
    };

    parsed.map_err(|_| format!("expected a decimal or 0x-prefixed hex 16-bit value, got `{raw}`"))
}

/// True when an unset filter accepts everything and a set one must match.
pub fn accepts(filter: Option<u16>, actual: u16) -> bool {
    match filter {
        Some(expected) => expected == actual,
        None => true,
    }
}

/// One device rendered as two lines: identity, then the path used to open it.
pub fn format_device(info: &DeviceInfo) -> String {
    let manufacturer = info.manufacturer.as_deref().unwrap_or("<unknown>");
    let product = info.product.as_deref().unwrap_or("<unnamed>");

    format!(
        "{:04x}:{:04x}  usage_page={:04x} usage={:04x} iface={}  {manufacturer} {product}\n    path={}",
        info.vendor_id, info.product_id, info.usage_page, info.usage, info.interface_number, info.path
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_parse_in_hex_and_decimal() {
        assert_eq!(parse_u16("0x046D"), Ok(0x046D));
        assert_eq!(parse_u16("0X046d"), Ok(0x046D));
        assert_eq!(parse_u16("1133"), Ok(0x046D));
        assert_eq!(parse_u16("  0xff00  "), Ok(0xFF00));
        assert_eq!(parse_u16("0"), Ok(0));
        assert_eq!(parse_u16("65535"), Ok(0xFFFF));
    }

    #[test]
    fn out_of_range_and_malformed_identifiers_are_rejected() {
        // 0x10000 and 65536 do not fit in a u16; silently truncating either
        // would match the wrong device.
        assert!(parse_u16("0x10000").is_err());
        assert!(parse_u16("65536").is_err());
        assert!(parse_u16("").is_err());
        assert!(parse_u16("nonsense").is_err());
        assert!(parse_u16("-1").is_err());
        assert!(
            parse_u16("046D").is_err(),
            "bare hex must not parse as decimal"
        );
    }

    #[test]
    fn an_unset_filter_accepts_every_value() {
        assert!(accepts(None, 0x046D));
        assert!(accepts(None, 0));
    }

    #[test]
    fn a_set_filter_accepts_only_its_own_value() {
        assert!(accepts(Some(0x046D), 0x046D));
        assert!(!accepts(Some(0x046D), 0x046E));
    }

    #[test]
    fn a_device_renders_with_its_open_path_and_padded_identifiers() {
        let info = DeviceInfo {
            path: "/dev/hidraw3".to_owned(),
            vendor_id: 0x046D,
            product_id: 0xC21C,
            usage_page: 0xFF00,
            usage: 0x0001,
            interface_number: 0,
            manufacturer: Some("Logitech".to_owned()),
            product: Some("G13".to_owned()),
            serial_number: None,
        };

        assert_eq!(
            format_device(&info),
            "046d:c21c  usage_page=ff00 usage=0001 iface=0  Logitech G13\n    path=/dev/hidraw3"
        );
    }

    #[test]
    fn missing_strings_render_as_placeholders_rather_than_blanks() {
        let rendered = format_device(&DeviceInfo::default());

        assert!(rendered.contains("<unknown> <unnamed>"), "{rendered}");
        assert!(rendered.contains("0000:0000"), "{rendered}");
    }
}
