//! Descriptor-driven input decode and output-report builders.
//!
//! Protocol numbers come from measured facts recorded in `docs/BOSUN-PLAN.md`.
//! This module does not name a vendor product.

use crate::descriptor::{DeviceDescriptor, KEY_BIT_COUNT};
use crate::error::CodecError;

/// Vendor input report ID.
pub const INPUT_REPORT_ID: u8 = 0x01;
/// Input report length including the report ID.
pub const INPUT_REPORT_LEN: usize = 8;

/// LCD output report ID.
pub const LCD_REPORT_ID: u8 = 0x03;
/// LCD output report length including the report ID.
pub const LCD_REPORT_LEN: usize = 992;
/// Padding bytes between the report ID and the framebuffer.
pub const LCD_PADDING: usize = 31;
/// Visible LCD rows. Rows at and above this stay zero in the packed buffer.
pub const LCD_VISIBLE_ROWS: usize = 43;
/// Packed storage rows (6 bands of 8).
pub const LCD_STORAGE_ROWS: usize = 48;
/// LCD columns.
pub const LCD_WIDTH: usize = 160;
/// Framebuffer bytes after padding.
pub const LCD_FRAMEBUFFER_LEN: usize = 960;

/// RGB backlight feature report ID.
pub const RGB_REPORT_ID: u8 = 0x07;
/// M-key LED feature report ID.
pub const LED_REPORT_ID: u8 = 0x05;

/// One edge produced by diffing the current report against the previous one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputEvent {
    KeyDown { name: String },
    KeyUp { name: String },
    Axis { name: String, value: u8 },
}

/// Stateful decoder: named keys for kept bits, axes from report bytes.
pub struct Decoder {
    key_by_bit: [Option<String>; KEY_BIT_COUNT],
    axes: Vec<(String, u8)>,
    previous_keys: Option<[bool; KEY_BIT_COUNT]>,
    previous_axes: Option<Vec<u8>>,
}

impl Decoder {
    /// Build a decoder from a validated descriptor.
    pub fn from_descriptor(descriptor: &DeviceDescriptor) -> std::result::Result<Self, CodecError> {
        let ignored: std::collections::HashSet<u8> =
            descriptor.caps.ignored_key_bits.iter().copied().collect();
        let mut key_by_bit = std::array::from_fn(|_| None);
        let mut next_key = 0usize;
        for (bit, slot) in key_by_bit.iter_mut().enumerate() {
            if ignored.contains(&(bit as u8)) {
                continue;
            }
            let name =
                descriptor.caps.keys.get(next_key).ok_or_else(|| {
                    CodecError::Descriptor("fewer key names than kept bits".into())
                })?;
            *slot = Some(name.clone());
            next_key += 1;
        }
        if next_key != descriptor.caps.keys.len() {
            return Err(CodecError::Descriptor(
                "more key names than kept bits".into(),
            ));
        }

        let axes = descriptor
            .caps
            .axes
            .iter()
            .map(|axis| (axis.id.clone(), axis.byte))
            .collect();

        Ok(Self {
            key_by_bit,
            axes,
            previous_keys: None,
            previous_axes: None,
        })
    }

    /// Decode one 8-byte report into edge events.
    pub fn decode(&mut self, report: &[u8]) -> std::result::Result<Vec<InputEvent>, CodecError> {
        if report.len() != INPUT_REPORT_LEN {
            return Err(CodecError::WrongLength(report.len()));
        }
        if report[0] != INPUT_REPORT_ID {
            return Err(CodecError::WrongReportId(report[0]));
        }

        let current_keys = key_bits(report);
        let mut events = Vec::new();

        match self.previous_keys {
            None => {
                for (bit, pressed) in current_keys.iter().enumerate() {
                    if *pressed {
                        if let Some(name) = &self.key_by_bit[bit] {
                            events.push(InputEvent::KeyDown { name: name.clone() });
                        }
                    }
                }
            }
            Some(previous) => {
                for (bit, (was, is)) in previous.iter().zip(current_keys.iter()).enumerate() {
                    if was == is {
                        continue;
                    }
                    let Some(name) = &self.key_by_bit[bit] else {
                        continue;
                    };
                    if *is {
                        events.push(InputEvent::KeyDown { name: name.clone() });
                    } else {
                        events.push(InputEvent::KeyUp { name: name.clone() });
                    }
                }
            }
        }
        self.previous_keys = Some(current_keys);

        let mut current_axes = Vec::with_capacity(self.axes.len());
        for (index, (name, byte)) in self.axes.iter().enumerate() {
            let byte = *byte as usize;
            if byte >= report.len() {
                return Err(CodecError::Descriptor(format!(
                    "axis {name} points at byte {byte}"
                )));
            }
            let value = report[byte];
            current_axes.push(value);
            let changed = match &self.previous_axes {
                None => true,
                Some(previous) => previous.get(index).copied() != Some(value),
            };
            if changed {
                events.push(InputEvent::Axis {
                    name: name.clone(),
                    value,
                });
            }
        }
        self.previous_axes = Some(current_axes);

        Ok(events)
    }
}

fn key_bits(report: &[u8]) -> [bool; KEY_BIT_COUNT] {
    let mut bits = [false; KEY_BIT_COUNT];
    for (bit, pressed) in bits.iter_mut().enumerate() {
        let byte = 3 + bit / 8;
        let mask = 1u8 << (bit % 8);
        *pressed = report[byte] & mask != 0;
    }
    bits
}

/// Feature report 7: global RGB backlight.
pub fn rgb_feature_report(red: u8, green: u8, blue: u8) -> [u8; 5] {
    [RGB_REPORT_ID, red, green, blue, 0x00]
}

/// Feature report 5: M1/M2/M3/MR LED mask.
pub fn leds_feature_report(mask: u8) -> [u8; 5] {
    [LED_REPORT_ID, mask, 0, 0, 0]
}

/// One LCD test frame: report ID 3, 31 zero padding bytes, visible border, hidden rows zero.
pub fn lcd_test_report() -> [u8; LCD_REPORT_LEN] {
    let mut report = [0u8; LCD_REPORT_LEN];
    report[0] = LCD_REPORT_ID;
    let framebuffer = &mut report[1 + LCD_PADDING..];
    for col in 0..LCD_WIDTH {
        set_visible_pixel(framebuffer, col, 0);
        set_visible_pixel(framebuffer, col, LCD_VISIBLE_ROWS - 1);
    }
    for row in 0..LCD_VISIBLE_ROWS {
        set_visible_pixel(framebuffer, 0, row);
        set_visible_pixel(framebuffer, LCD_WIDTH - 1, row);
    }
    report
}

fn set_visible_pixel(framebuffer: &mut [u8], col: usize, row: usize) {
    debug_assert!(row < LCD_VISIBLE_ROWS);
    debug_assert!(col < LCD_WIDTH);
    let offset = col + (row >> 3) * LCD_WIDTH;
    let bit = 1u8 << (row & 7);
    framebuffer[offset] |= bit;
}

/// True when every packed bit for rows 43–47 is clear.
pub fn hidden_lcd_rows_are_zero(report: &[u8]) -> bool {
    if report.len() != LCD_REPORT_LEN {
        return false;
    }
    let framebuffer = &report[1 + LCD_PADDING..];
    for row in LCD_VISIBLE_ROWS..LCD_STORAGE_ROWS {
        for col in 0..LCD_WIDTH {
            let offset = col + (row >> 3) * LCD_WIDTH;
            let bit = 1u8 << (row & 7);
            if framebuffer[offset] & bit != 0 {
                return false;
            }
        }
    }
    true
}

/// Format one recorded 8-byte report as lowercase hex.
pub fn format_recorded_report(report: &[u8; INPUT_REPORT_LEN]) -> String {
    report
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Parse one recorded hex line into an 8-byte report.
pub fn parse_recorded_report(
    line: &str,
) -> std::result::Result<[u8; INPUT_REPORT_LEN], CodecError> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return Err(CodecError::InvalidRecording(line.to_owned()));
    }
    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    if parts.len() != INPUT_REPORT_LEN {
        return Err(CodecError::InvalidRecording(line.to_owned()));
    }
    let mut report = [0u8; INPUT_REPORT_LEN];
    for (index, part) in parts.iter().enumerate() {
        report[index] = u8::from_str_radix(part, 16)
            .map_err(|_| CodecError::InvalidRecording(line.to_owned()))?;
    }
    Ok(report)
}

/// Parse a recorded fixture file, skipping blanks and `#` comments.
pub fn parse_recorded_reports(
    text: &str,
) -> std::result::Result<Vec<[u8; INPUT_REPORT_LEN]>, CodecError> {
    let mut reports = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        reports.push(parse_recorded_report(trimmed)?);
    }
    Ok(reports)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptor::parse_descriptor;
    use std::fs;
    use std::path::PathBuf;

    fn g13_decoder() -> Decoder {
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../devices/logitech-g13.toml");
        let descriptor = crate::descriptor::load_descriptor(path).unwrap();
        Decoder::from_descriptor(&descriptor).unwrap()
    }

    fn rest() -> [u8; 8] {
        [0x01, 0x7F, 0x7F, 0, 0, 0, 0, 0]
    }

    fn fixture_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/g13")
    }

    fn load_hex(name: &str) -> [u8; 8] {
        let text = fs::read_to_string(fixture_dir().join(name)).unwrap();
        parse_recorded_report(text.lines().next().unwrap()).unwrap()
    }

    #[test]
    fn a_pressed_kept_bit_emits_a_named_key_down() {
        let mut decoder = g13_decoder();
        let events = decoder.decode(&load_hex("G1.hex")).unwrap();
        assert!(
            events.contains(&InputEvent::KeyDown { name: "G1".into() }),
            "{events:?}"
        );
    }

    #[test]
    fn releasing_a_key_emits_up_after_a_diff() {
        let mut decoder = g13_decoder();
        decoder.decode(&load_hex("G1.hex")).unwrap();
        let events = decoder.decode(&rest()).unwrap();
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, InputEvent::KeyUp { name } if name == "G1"))
                .count(),
            1,
            "{events:?}"
        );
    }

    #[test]
    fn ignored_bits_emit_no_key_events() {
        let mut decoder = g13_decoder();
        decoder.decode(&rest()).unwrap();

        let ignored = [
            (22, 5, 1u8 << 6),
            (23, 5, 1u8 << 7),
            (36, 7, 1u8 << 4),
            (37, 7, 1u8 << 5),
            (38, 7, 1u8 << 6),
            (39, 7, 1u8 << 7),
        ];

        for (bit, byte, mask) in ignored {
            let mut report = rest();
            report[byte] |= mask;
            let events = decoder.decode(&report).unwrap();
            let key_events: Vec<_> = events
                .iter()
                .filter(|event| {
                    matches!(event, InputEvent::KeyDown { .. } | InputEvent::KeyUp { .. })
                })
                .collect();
            assert!(
                key_events.is_empty(),
                "bit {bit} produced key events: {events:?}"
            );
            decoder.decode(&rest()).unwrap();
        }
    }

    #[test]
    fn stick_bytes_decode_to_0_255_axes() {
        let mut decoder = g13_decoder();
        let events = decoder.decode(&[0x01, 0x00, 0xFF, 0, 0, 0, 0, 0]).unwrap();
        assert!(events.contains(&InputEvent::Axis {
            name: "stick-x".into(),
            value: 0
        }));
        assert!(events.contains(&InputEvent::Axis {
            name: "stick-y".into(),
            value: 255
        }));
    }

    #[test]
    fn golden_key_fixtures_each_emit_exactly_that_name() {
        let names = [
            "G1", "G2", "G3", "G4", "G5", "G6", "G7", "G8", "G9", "G10", "G11", "G12", "G13",
            "G14", "G15", "G16", "G17", "G18", "G19", "G20", "G21", "G22", "BD", "L1", "L2", "L3",
            "L4", "M1", "M2", "M3", "MR", "LEFT", "DOWN", "TOP",
        ];
        for name in names {
            let mut decoder = g13_decoder();
            let events = decoder.decode(&load_hex(&format!("{name}.hex"))).unwrap();
            let downs: Vec<_> = events
                .iter()
                .filter_map(|event| match event {
                    InputEvent::KeyDown { name } => Some(name.as_str()),
                    _ => None,
                })
                .collect();
            assert_eq!(downs, [name], "{name} fixture: {events:?}");
        }
    }

    #[test]
    fn golden_stick_fixtures_decode_their_axis_bytes() {
        let cases = [
            ("stick-center.hex", 0x7F, 0x7F),
            ("stick-north.hex", 0x7F, 0x00),
            ("stick-south.hex", 0x7F, 0xFF),
            ("stick-west.hex", 0x00, 0x7F),
            ("stick-east.hex", 0xFF, 0x7F),
            ("stick-nw.hex", 0x00, 0x00),
            ("stick-ne.hex", 0xFF, 0x00),
            ("stick-sw.hex", 0x00, 0xFF),
            ("stick-se.hex", 0xFF, 0xFF),
            ("stick-diag-nw.hex", 0x40, 0x40),
            ("stick-diag-ne.hex", 0xC0, 0x40),
            ("stick-diag-sw.hex", 0x40, 0xC0),
            ("stick-diag-se.hex", 0xC0, 0xC0),
        ];
        for (file, x, y) in cases {
            let mut decoder = g13_decoder();
            let report = load_hex(file);
            assert_eq!(report[1], x, "{file}");
            assert_eq!(report[2], y, "{file}");
            let events = decoder.decode(&report).unwrap();
            assert!(
                events.contains(&InputEvent::Axis {
                    name: "stick-x".into(),
                    value: x
                }),
                "{file}: {events:?}"
            );
            assert!(
                events.contains(&InputEvent::Axis {
                    name: "stick-y".into(),
                    value: y
                }),
                "{file}: {events:?}"
            );
        }
    }

    #[test]
    fn rgb_and_led_builders_match_the_measured_layout() {
        assert_eq!(
            rgb_feature_report(0xFF, 0x00, 0x11),
            [0x07, 0xFF, 0x00, 0x11, 0x00]
        );
        assert_eq!(leds_feature_report(0x0A), [0x05, 0x0A, 0, 0, 0]);
    }

    #[test]
    fn lcd_test_report_has_the_measured_header_and_hidden_rows() {
        let report = lcd_test_report();
        assert_eq!(report.len(), 992);
        assert_eq!(report[0], 0x03);
        assert!(report[1..=31].iter().all(|byte| *byte == 0));
        assert!(hidden_lcd_rows_are_zero(&report));
        assert_ne!(report[32..].iter().filter(|byte| **byte != 0).count(), 0);
    }

    #[test]
    fn recorded_hex_round_trips() {
        let report = [0x01, 0x7F, 0x80, 0x01, 0, 0, 0, 0];
        let line = format_recorded_report(&report);
        assert_eq!(parse_recorded_report(&line).unwrap(), report);
    }

    #[test]
    fn a_document_without_ignored_bits_cannot_build_a_40_bit_codec() {
        let text = r#"
id = "short"
class = "A"
[match]
vid = 1
pid = 2
usage_page = 3
[caps]
keys = ["ONLY"]
ignored_key_bits = []
[[caps.axes]]
id = "x"
byte = 1
bits = 8
[caps.screen]
width = 1
height = 1
storage_height = 1
format = "1bpp-bands"
report_id = 3
padding = 31
[caps.rgb]
zones = 1
report_id = 7
[caps.leds]
names = ["M1"]
report_id = 5
"#;
        assert!(parse_descriptor(text).is_err());
    }
}
