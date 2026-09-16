//! Device descriptors: TOML data, not compiled product policy.
//!
//! A descriptor names match criteria and capabilities. The G13 lives in
//! `devices/logitech-g13.toml`. This module does not mention that product.

use std::collections::HashSet;
use std::path::Path;

use serde::Deserialize;

use crate::device::DeviceMatch;
use crate::error::DescriptorError;

/// Bits in the 5-byte key field (bytes 3–7 of an 8-byte input report).
pub const KEY_BIT_COUNT: usize = 40;

/// One loaded device document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceDescriptor {
    pub id: String,
    pub class: String,
    pub match_criteria: MatchCriteria,
    pub caps: Capabilities,
}

/// VID, PID, and usage page used to select an interface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub struct MatchCriteria {
    pub vid: u16,
    pub pid: u16,
    pub usage_page: u16,
}

impl MatchCriteria {
    /// Criteria in the form [`DeviceMatch`] expects.
    pub fn device_match(self) -> DeviceMatch {
        DeviceMatch::new(self.vid, self.pid, self.usage_page)
    }
}

/// Controls and output surfaces declared by the descriptor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Capabilities {
    pub keys: Vec<String>,
    pub ignored_key_bits: Vec<u8>,
    pub axes: Vec<Axis>,
    pub screen: Screen,
    pub rgb: Rgb,
    pub leds: Leds,
}

/// One analog axis packed into an input-report byte.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Axis {
    pub id: String,
    pub byte: u8,
    pub bits: u8,
}

/// Monochrome LCD packing parameters.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Screen {
    pub width: u32,
    pub height: u32,
    pub storage_height: u32,
    pub format: String,
    pub report_id: u8,
    pub padding: u8,
}

/// Global RGB backlight.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Rgb {
    pub zones: u8,
    pub report_id: u8,
}

/// Named mode LEDs.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Leds {
    pub names: Vec<String>,
    pub report_id: u8,
}

#[derive(Deserialize)]
struct RawDescriptor {
    id: String,
    class: String,
    #[serde(rename = "match")]
    match_criteria: MatchCriteria,
    caps: RawCaps,
}

#[derive(Deserialize)]
struct RawCaps {
    keys: KeysField,
    #[serde(default)]
    ignored_key_bits: Vec<u8>,
    axes: Vec<Axis>,
    screen: Screen,
    rgb: Rgb,
    leds: Leds,
}

/// Accepts only a literal array. A string (including range syntax) is a type error.
#[derive(Deserialize)]
#[serde(untagged)]
enum KeysField {
    Literals(Vec<String>),
    RangeSyntax(String),
}

/// Load and validate a descriptor file.
pub fn load_descriptor(
    path: impl AsRef<Path>,
) -> std::result::Result<DeviceDescriptor, DescriptorError> {
    let path = path.as_ref();
    let text = std::fs::read_to_string(path).map_err(|source| DescriptorError::Io {
        path: path.display().to_string(),
        source,
    })?;
    parse_descriptor(&text)
}

/// Parse and validate a descriptor document.
pub fn parse_descriptor(text: &str) -> std::result::Result<DeviceDescriptor, DescriptorError> {
    let raw: RawDescriptor =
        toml::from_str(text).map_err(|error| DescriptorError::Malformed(error.to_string()))?;
    validate(raw)
}

fn validate(raw: RawDescriptor) -> std::result::Result<DeviceDescriptor, DescriptorError> {
    let keys = match raw.caps.keys {
        KeysField::RangeSyntax(syntax) => {
            let reason = if syntax.contains("..") {
                "range syntax is rejected"
            } else {
                "caps.keys must be a literal array"
            };
            return Err(DescriptorError::Malformed(format!(
                "{reason}; use a literal key array, got {syntax:?}"
            )));
        }
        KeysField::Literals(keys) => keys,
    };

    if raw.id.is_empty() {
        return Err(DescriptorError::Malformed("id must not be empty".into()));
    }
    if keys.is_empty() {
        return Err(DescriptorError::Malformed(
            "caps.keys must not be empty".into(),
        ));
    }

    let mut seen = HashSet::new();
    for key in &keys {
        if key.is_empty() {
            return Err(DescriptorError::Malformed(
                "key names must not be empty".into(),
            ));
        }
        if key.contains("..") {
            return Err(DescriptorError::Malformed(format!(
                "range syntax is rejected; use a literal key array, got {key:?}"
            )));
        }
        if !seen.insert(key) {
            return Err(DescriptorError::Malformed(format!(
                "duplicate key name {key}"
            )));
        }
    }

    let mut ignored_seen = HashSet::new();
    for bit in &raw.caps.ignored_key_bits {
        if *bit as usize >= KEY_BIT_COUNT {
            return Err(DescriptorError::Malformed(format!(
                "ignored key bit {bit} is outside 0..{KEY_BIT_COUNT}"
            )));
        }
        if !ignored_seen.insert(*bit) {
            return Err(DescriptorError::Malformed(format!(
                "duplicate ignored key bit {bit}"
            )));
        }
    }

    if keys.len() + raw.caps.ignored_key_bits.len() != KEY_BIT_COUNT {
        return Err(DescriptorError::Malformed(format!(
            "caps.keys ({}) plus ignored_key_bits ({}) must cover {KEY_BIT_COUNT} bits",
            keys.len(),
            raw.caps.ignored_key_bits.len()
        )));
    }

    for axis in &raw.caps.axes {
        if axis.id.is_empty() {
            return Err(DescriptorError::Malformed(
                "axis id must not be empty".into(),
            ));
        }
        if axis.bits == 0 {
            return Err(DescriptorError::Malformed(format!(
                "axis {} must declare a positive bit width",
                axis.id
            )));
        }
    }

    if raw.caps.screen.width == 0 || raw.caps.screen.height == 0 {
        return Err(DescriptorError::Malformed(
            "screen width and height must be positive".into(),
        ));
    }
    if raw.caps.screen.storage_height < raw.caps.screen.height {
        return Err(DescriptorError::Malformed(
            "screen storage_height must be at least height".into(),
        ));
    }

    Ok(DeviceDescriptor {
        id: raw.id,
        class: raw.class,
        match_criteria: raw.match_criteria,
        caps: Capabilities {
            keys,
            ignored_key_bits: raw.caps.ignored_key_bits,
            axes: raw.caps.axes,
            screen: raw.caps.screen,
            rgb: raw.caps.rgb,
            leds: raw.caps.leds,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn committed_g13() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../devices/logitech-g13.toml")
    }

    const VALID: &str = r#"
id = "example-pad"
class = "A"

[match]
vid = 0x1234
pid = 0x5678
usage_page = 0xFF00

[caps]
keys = [
  "K0", "K1", "K2", "K3", "K4", "K5", "K6", "K7", "K8", "K9",
  "K10", "K11", "K12", "K13", "K14", "K15", "K16", "K17", "K18", "K19",
  "K20", "K21", "K22", "K23", "K24", "K25", "K26", "K27", "K28", "K29",
  "K30", "K31", "K32", "K33"
]
ignored_key_bits = [22, 23, 36, 37, 38, 39]

[[caps.axes]]
id = "stick-x"
byte = 1
bits = 8

[[caps.axes]]
id = "stick-y"
byte = 2
bits = 8

[caps.screen]
width = 160
height = 43
storage_height = 48
format = "1bpp-bands"
report_id = 3
padding = 31

[caps.rgb]
zones = 1
report_id = 7

[caps.leds]
names = ["M1", "M2", "M3", "MR"]
report_id = 5
"#;

    #[test]
    fn the_committed_g13_descriptor_loads_with_34_literal_keys() {
        let descriptor = load_descriptor(committed_g13()).expect("committed descriptor is valid");

        assert_eq!(descriptor.id, "logitech-g13");
        assert_eq!(descriptor.caps.keys.len(), 34);
        assert_eq!(
            descriptor.caps.keys,
            [
                "G1", "G2", "G3", "G4", "G5", "G6", "G7", "G8", "G9", "G10", "G11", "G12", "G13",
                "G14", "G15", "G16", "G17", "G18", "G19", "G20", "G21", "G22", "BD", "L1", "L2",
                "L3", "L4", "M1", "M2", "M3", "MR", "LEFT", "DOWN", "TOP"
            ]
        );
        assert_eq!(
            descriptor.match_criteria,
            MatchCriteria {
                vid: 0x046D,
                pid: 0xC21C,
                usage_page: 0xFF00,
            }
        );
        assert_eq!(descriptor.caps.axes.len(), 2);
        assert_eq!(descriptor.caps.screen.width, 160);
        assert_eq!(descriptor.caps.rgb.report_id, 7);
        assert_eq!(descriptor.caps.leds.names, ["M1", "M2", "M3", "MR"]);
        assert_eq!(descriptor.caps.ignored_key_bits, [22, 23, 36, 37, 38, 39]);
    }

    #[test]
    fn a_valid_document_round_trips_through_parse() {
        let descriptor = parse_descriptor(VALID).expect("fixture is valid");
        assert_eq!(descriptor.caps.keys.len(), 34);
        assert_eq!(descriptor.match_criteria.device_match().vendor_id, 0x1234);
    }

    #[test]
    fn a_missing_field_is_an_error_not_a_panic() {
        let missing_pid = VALID.replace("pid = 0x5678\n", "");
        let error = parse_descriptor(&missing_pid).expect_err("pid is required");
        assert!(matches!(error, DescriptorError::Malformed(_)), "{error:?}");
    }

    #[test]
    fn a_wrong_type_is_an_error_not_a_panic() {
        let wrong_type = VALID.replace("vid = 0x1234", "vid = \"046D\"");
        let error = parse_descriptor(&wrong_type).expect_err("vid must be an integer");
        assert!(matches!(error, DescriptorError::Malformed(_)), "{error:?}");
    }

    #[test]
    fn range_syntax_as_a_string_is_rejected() {
        let ranged = VALID.replace(
            "keys = [\n  \"K0\", \"K1\", \"K2\", \"K3\", \"K4\", \"K5\", \"K6\", \"K7\", \"K8\", \"K9\",\n  \"K10\", \"K11\", \"K12\", \"K13\", \"K14\", \"K15\", \"K16\", \"K17\", \"K18\", \"K19\",\n  \"K20\", \"K21\", \"K22\", \"K23\", \"K24\", \"K25\", \"K26\", \"K27\", \"K28\", \"K29\",\n  \"K30\", \"K31\", \"K32\", \"K33\"\n]",
            "keys = \"K0..=K33\"",
        );
        let error = parse_descriptor(&ranged).expect_err("range syntax is rejected");
        assert!(error.to_string().contains("range syntax"), "{error}");
    }

    #[test]
    fn range_syntax_inside_the_array_is_rejected() {
        let ranged = VALID.replace("\"K0\"", "\"K0..=K3\"");
        let error = parse_descriptor(&ranged).expect_err("range tokens are rejected");
        assert!(error.to_string().contains("range syntax"), "{error}");
    }

    #[test]
    fn malformed_descriptors_do_not_panic() {
        let out_of_range = VALID.replace(
            "ignored_key_bits = [22, 23, 36, 37, 38, 39]",
            "ignored_key_bits = [99]",
        );
        for document in ["", "id = 1", "keys = \"G1..=G22\"", out_of_range.as_str()] {
            let result = std::panic::catch_unwind(|| parse_descriptor(document));
            let parsed = result.expect("parse_descriptor must not panic");
            assert!(parsed.is_err(), "expected Err for {document:?}");
        }
    }
}
