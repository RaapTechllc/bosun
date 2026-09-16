//! `bosunctl` library surface so device commands can run over any [`Transport`].

mod listing;

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use bosun_hid::{
    format_recorded_report, hidden_lcd_rows_are_zero, lcd_test_report, leds_feature_report,
    load_descriptor, rgb_feature_report, Decoder, DeviceDescriptor, DeviceInfo, HidError,
    HidTransport, InputEvent, ReadOutcome, Transport, LCD_REPORT_LEN, RGB_REPORT_ID,
};
use clap::{Args, Parser, Subcommand};

use crate::listing::{accepts, format_device, parse_u16};

/// Default descriptor path relative to the process working directory.
pub const DEFAULT_DESCRIPTOR: &str = "devices/logitech-g13.toml";

#[derive(Debug, Parser)]
#[command(name = "bosunctl", version, about = "Bosun control CLI")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Inspect and exercise HID devices.
    Device {
        #[command(subcommand)]
        command: DeviceCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum DeviceCommand {
    /// List enumerated HID interfaces, optionally filtered.
    List(ListArgs),
    /// Print a loaded device descriptor.
    Info(DescriptorArgs),
    /// Print decoded key and axis edge events.
    Watch(WatchArgs),
    /// Write raw 8-byte input reports to a fixture file.
    Record(RecordArgs),
    /// Set the global RGB backlight and read it back.
    Rgb(RgbArgs),
    /// Set the M-key LED mask.
    Leds(LedsArgs),
    /// Send one LCD test frame.
    Lcd {
        #[command(subcommand)]
        command: LcdCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum LcdCommand {
    /// Write a fixed visible-border frame (no widgets).
    Test(DeviceIoArgs),
}

#[derive(Debug, Args)]
pub struct ListArgs {
    /// Vendor ID, decimal or 0x-prefixed hex.
    #[arg(long, value_parser = parse_u16)]
    pub vid: Option<u16>,

    /// Product ID, decimal or 0x-prefixed hex.
    #[arg(long, value_parser = parse_u16)]
    pub pid: Option<u16>,

    /// HID usage page, decimal or 0x-prefixed hex.
    #[arg(long = "usage-page", value_parser = parse_u16)]
    pub usage_page: Option<u16>,
}

impl ListArgs {
    fn selects(&self, info: &DeviceInfo) -> bool {
        accepts(self.vid, info.vendor_id)
            && accepts(self.pid, info.product_id)
            && accepts(self.usage_page, info.usage_page)
    }
}

#[derive(Debug, Args)]
pub struct DescriptorArgs {
    /// Path to a device TOML descriptor.
    #[arg(long, default_value = DEFAULT_DESCRIPTOR)]
    pub descriptor: PathBuf,
}

#[derive(Debug, Args)]
pub struct DeviceIoArgs {
    /// Path to a device TOML descriptor.
    #[arg(long, default_value = DEFAULT_DESCRIPTOR)]
    pub descriptor: PathBuf,

    /// Open this enumerated path instead of matching the descriptor.
    #[arg(long)]
    pub path: Option<String>,
}

#[derive(Debug, Args)]
pub struct WatchArgs {
    #[command(flatten)]
    pub io: DeviceIoArgs,

    /// Stop after this many decoded events (tests and finite captures).
    #[arg(long)]
    pub max_events: Option<usize>,
}

#[derive(Debug, Args)]
pub struct RecordArgs {
    #[command(flatten)]
    pub io: DeviceIoArgs,

    /// Fixture file to write.
    #[arg(long, short)]
    pub output: PathBuf,

    /// Stop after this many reports.
    #[arg(long)]
    pub max_reports: Option<usize>,
}

#[derive(Debug, Args)]
pub struct RgbArgs {
    #[command(flatten)]
    pub io: DeviceIoArgs,
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

#[derive(Debug, Args)]
pub struct LedsArgs {
    #[command(flatten)]
    pub io: DeviceIoArgs,
    /// LED mask, decimal or 0x-prefixed hex.
    #[arg(value_parser = parse_u8)]
    pub mask: u8,
}

/// Parse an 8-bit identifier written in decimal or `0x`-prefixed hex.
pub fn parse_u8(raw: &str) -> std::result::Result<u8, String> {
    let text = raw.trim();
    let parsed = match text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        Some(hex) => u8::from_str_radix(hex, 16),
        None => text.parse::<u8>(),
    };
    parsed.map_err(|_| format!("expected a decimal or 0x-prefixed hex 8-bit value, got `{raw}`"))
}

/// Limits for [`watch_events`] so tests do not block.
#[derive(Clone, Debug)]
pub struct WatchLimits {
    pub read_timeout: Duration,
    pub max_events: Option<usize>,
    pub exit_if_reopen_fails: bool,
}

impl Default for WatchLimits {
    fn default() -> Self {
        Self {
            read_timeout: Duration::from_millis(100),
            max_events: None,
            exit_if_reopen_fails: false,
        }
    }
}

/// Run the parsed CLI against the live HID backend.
pub fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Device { command } => match command {
            DeviceCommand::List(args) => device_list(&args),
            DeviceCommand::Info(args) => {
                let descriptor = load_required_descriptor(&args.descriptor)?;
                info(&descriptor, &mut io::stdout())
            }
            DeviceCommand::Watch(args) => {
                let descriptor = load_required_descriptor(&args.io.descriptor)?;
                let mut decoder = Decoder::from_descriptor(&descriptor)?;
                let limits = WatchLimits {
                    max_events: args.max_events,
                    ..WatchLimits::default()
                };
                watch_events(
                    || open_live(&descriptor, args.io.path.as_deref()),
                    &mut decoder,
                    &mut io::stdout(),
                    &limits,
                )
            }
            DeviceCommand::Record(args) => {
                let descriptor = load_required_descriptor(&args.io.descriptor)?;
                let mut file = std::fs::File::create(&args.output)
                    .with_context(|| format!("could not create {}", args.output.display()))?;
                record_reports(
                    || open_live(&descriptor, args.io.path.as_deref()),
                    &mut file,
                    args.max_reports,
                    Duration::from_millis(100),
                    false,
                )?;
                Ok(())
            }
            DeviceCommand::Rgb(args) => {
                let descriptor = load_required_descriptor(&args.io.descriptor)?;
                let mut transport = open_live(&descriptor, args.io.path.as_deref())?;
                rgb(
                    &mut transport,
                    args.red,
                    args.green,
                    args.blue,
                    &mut io::stdout(),
                )
            }
            DeviceCommand::Leds(args) => {
                let descriptor = load_required_descriptor(&args.io.descriptor)?;
                let mut transport = open_live(&descriptor, args.io.path.as_deref())?;
                leds(&mut transport, args.mask, &mut io::stdout())
            }
            DeviceCommand::Lcd {
                command: LcdCommand::Test(args),
            } => {
                let descriptor = load_required_descriptor(&args.descriptor)?;
                let mut transport = open_live(&descriptor, args.path.as_deref())?;
                lcd_test(&mut transport, &mut io::stdout())
            }
        },
    }
}

fn load_required_descriptor(path: &Path) -> Result<DeviceDescriptor> {
    load_descriptor(path)
        .with_context(|| format!("could not load device descriptor {}", path.display()))
}

fn open_live(descriptor: &DeviceDescriptor, path: Option<&str>) -> bosun_hid::Result<HidTransport> {
    let api = HidTransport::api()?;
    match path {
        Some(path) => HidTransport::open_path(&api, path),
        None => HidTransport::open(&api, &descriptor.match_criteria.device_match()),
    }
}

fn device_list(args: &ListArgs) -> Result<()> {
    let api = HidTransport::api().context("could not initialise the HID backend")?;
    let devices = HidTransport::enumerate(&api);
    let matched: Vec<&DeviceInfo> = devices.iter().filter(|info| args.selects(info)).collect();

    if matched.is_empty() {
        println!("No HID interface matched. {} enumerated.", devices.len());
        return Ok(());
    }

    for info in matched {
        println!("{}", format_device(info));
    }

    Ok(())
}

/// Print descriptor identity and capabilities.
pub fn info(descriptor: &DeviceDescriptor, out: &mut impl Write) -> Result<()> {
    writeln!(out, "id={}", descriptor.id)?;
    writeln!(out, "class={}", descriptor.class)?;
    writeln!(
        out,
        "match={:04x}:{:04x} usage_page={:04x}",
        descriptor.match_criteria.vid,
        descriptor.match_criteria.pid,
        descriptor.match_criteria.usage_page
    )?;
    writeln!(out, "keys={}", descriptor.caps.keys.len())?;
    writeln!(out, "  {}", descriptor.caps.keys.join(" "))?;
    for axis in &descriptor.caps.axes {
        writeln!(
            out,
            "axis {} byte={} bits={}",
            axis.id, axis.byte, axis.bits
        )?;
    }
    writeln!(
        out,
        "screen {}x{} storage={} report={} padding={}",
        descriptor.caps.screen.width,
        descriptor.caps.screen.height,
        descriptor.caps.screen.storage_height,
        descriptor.caps.screen.report_id,
        descriptor.caps.screen.padding
    )?;
    writeln!(out, "rgb report={}", descriptor.caps.rgb.report_id)?;
    writeln!(
        out,
        "leds {} report={}",
        descriptor.caps.leds.names.join(","),
        descriptor.caps.leds.report_id
    )?;
    Ok(())
}

/// Format one decoded event for `watch`.
pub fn format_event(event: &InputEvent) -> String {
    match event {
        InputEvent::KeyDown { name } => format!("key {name} down"),
        InputEvent::KeyUp { name } => format!("key {name} up"),
        InputEvent::Axis { name, value } => format!("axis {name} {value}"),
    }
}

/// Read reports, decode them, and print events. Re-opens after disconnect.
pub fn watch_events<T, F, W>(
    mut opener: F,
    decoder: &mut Decoder,
    out: &mut W,
    limits: &WatchLimits,
) -> Result<()>
where
    T: Transport,
    F: FnMut() -> bosun_hid::Result<T>,
    W: Write,
{
    let mut transport = opener().context("could not open the device")?;
    let mut events_seen = 0usize;

    loop {
        if limits.max_events.is_some_and(|max| events_seen >= max) {
            return Ok(());
        }

        let mut buf = [0u8; 8];
        match transport.read(&mut buf, limits.read_timeout) {
            Ok(ReadOutcome::Timeout) => {}
            Ok(ReadOutcome::Report(len)) => {
                let events = decoder
                    .decode(&buf[..len])
                    .context("could not decode input report")?;
                for event in events {
                    writeln!(out, "{}", format_event(&event))?;
                    events_seen += 1;
                    if limits.max_events.is_some_and(|max| events_seen >= max) {
                        return Ok(());
                    }
                }
            }
            Err(HidError::Disconnected) => match opener() {
                Ok(next) => {
                    transport = next;
                    writeln!(out, "reconnect")?;
                }
                Err(_) if limits.exit_if_reopen_fails => return Ok(()),
                Err(_) => {
                    // Live unplug: retry without a long sleep so replug stays under 2s.
                    std::thread::sleep(Duration::from_millis(50));
                }
            },
            Err(error) => return Err(error.into()),
        }
    }
}

/// Write raw 8-byte reports as hex lines.
pub fn record_reports<T, F, W>(
    mut opener: F,
    out: &mut W,
    max_reports: Option<usize>,
    read_timeout: Duration,
    exit_if_reopen_fails: bool,
) -> Result<usize>
where
    T: Transport,
    F: FnMut() -> bosun_hid::Result<T>,
    W: Write,
{
    let mut transport = opener().context("could not open the device")?;
    let mut written = 0usize;

    loop {
        if max_reports.is_some_and(|max| written >= max) {
            return Ok(written);
        }

        let mut buf = [0u8; 8];
        match transport.read(&mut buf, read_timeout) {
            Ok(ReadOutcome::Timeout) => {
                if max_reports.is_some() {
                    continue;
                }
            }
            Ok(ReadOutcome::Report(len)) => {
                if len != 8 {
                    bail!("record expects 8-byte reports, got {len}");
                }
                let mut report = [0u8; 8];
                report.copy_from_slice(&buf[..8]);
                writeln!(out, "{}", format_recorded_report(&report))?;
                written += 1;
            }
            Err(HidError::Disconnected) => {
                if exit_if_reopen_fails {
                    return Ok(written);
                }
                transport = opener().context("could not reopen the device for record")?;
            }
            Err(error) => return Err(error.into()),
        }
    }
}

/// Send feature report 7 and print the readback.
pub fn rgb<T: Transport>(
    transport: &mut T,
    red: u8,
    green: u8,
    blue: u8,
    out: &mut impl Write,
) -> Result<()> {
    let report = rgb_feature_report(red, green, blue);
    transport
        .send_feature_report(&report)
        .context("could not send RGB feature report")?;
    let mut readback = [0u8; 5];
    readback[0] = RGB_REPORT_ID;
    let len = transport
        .get_feature_report(&mut readback)
        .context("could not read RGB feature report")?;
    writeln!(
        out,
        "rgb sent {:02x?} readback {:02x?}",
        report,
        &readback[..len]
    )?;
    Ok(())
}

/// Send feature report 5.
pub fn leds<T: Transport>(transport: &mut T, mask: u8, out: &mut impl Write) -> Result<()> {
    let report = leds_feature_report(mask);
    transport
        .send_feature_report(&report)
        .context("could not send LED feature report")?;
    writeln!(out, "leds sent {report:02x?}")?;
    Ok(())
}

/// Send one 992-byte LCD test report.
pub fn lcd_test<T: Transport>(transport: &mut T, out: &mut impl Write) -> Result<()> {
    let report = lcd_test_report();
    if report.len() != LCD_REPORT_LEN || report[0] != 0x03 {
        bail!("LCD test report is malformed");
    }
    if !report[1..=31].iter().all(|byte| *byte == 0) {
        bail!("LCD test padding must be zero");
    }
    if !hidden_lcd_rows_are_zero(&report) {
        bail!("LCD hidden rows 43-47 must be zero");
    }
    let written = transport
        .write(&report)
        .context("could not write LCD test report")?;
    writeln!(out, "lcd test wrote {written} bytes report={}", report[0])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    use bosun_hid::{load_descriptor, parse_recorded_reports, DeviceInfo, MockTransport};
    use clap::CommandFactory;
    use std::io::Cursor;
    use std::time::Instant;

    fn g13() -> DeviceDescriptor {
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../devices/logitech-g13.toml");
        load_descriptor(path).unwrap()
    }

    fn mock() -> MockTransport {
        MockTransport::new(DeviceInfo {
            path: "mock".into(),
            vendor_id: 0x1234,
            product_id: 0x5678,
            usage_page: 0xFF00,
            ..DeviceInfo::default()
        })
    }

    fn g1_report() -> [u8; 8] {
        [0x01, 0x7F, 0x7F, 0x01, 0, 0, 0, 0]
    }

    #[test]
    fn the_cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn identifiers_accept_hex_on_the_command_line() {
        let cli = Cli::parse_from([
            "bosunctl",
            "device",
            "list",
            "--vid",
            "0x046D",
            "--pid",
            "0xC21C",
            "--usage-page",
            "0xFF00",
        ]);
        match cli.command {
            Command::Device {
                command: DeviceCommand::List(args),
            } => {
                assert_eq!(args.vid, Some(0x046D));
                assert_eq!(args.pid, Some(0xC21C));
                assert_eq!(args.usage_page, Some(0xFF00));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn an_unfiltered_list_selects_everything() {
        let args = ListArgs {
            vid: None,
            pid: None,
            usage_page: None,
        };
        assert!(args.selects(&DeviceInfo::default()));
    }

    #[test]
    fn a_usage_page_filter_excludes_the_other_collections_of_one_device() {
        let args = ListArgs {
            vid: Some(0x046D),
            pid: Some(0xC21C),
            usage_page: Some(0xFF00),
        };
        let vendor = DeviceInfo {
            vendor_id: 0x046D,
            product_id: 0xC21C,
            usage_page: 0xFF00,
            ..DeviceInfo::default()
        };
        assert!(args.selects(&vendor));
        assert!(!args.selects(&DeviceInfo {
            usage_page: 0x0001,
            ..vendor
        }));
    }

    #[test]
    fn a_malformed_identifier_is_rejected_rather_than_ignored() {
        assert!(Cli::try_parse_from(["bosunctl", "device", "list", "--vid", "zzz"]).is_err());
        assert!(Cli::try_parse_from(["bosunctl", "device", "list", "--vid", "0x10000"]).is_err());
    }

    #[test]
    fn info_prints_the_loaded_descriptor() {
        let descriptor = g13();
        let mut out = Vec::new();
        info(&descriptor, &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("id=logitech-g13"), "{text}");
        assert!(text.contains("046d:c21c"), "{text}");
        assert!(text.contains("G1"), "{text}");
        assert!(text.contains("LEFT DOWN TOP"), "{text}");
        assert!(text.contains("stick-x"), "{text}");
        assert!(text.contains("rgb report=7"), "{text}");
        assert!(text.contains("leds M1,M2,M3,MR report=5"), "{text}");
    }

    #[test]
    fn watch_prints_named_events_over_mock_transport() {
        let descriptor = g13();
        let mut decoder = Decoder::from_descriptor(&descriptor).unwrap();
        let mut out = Vec::new();
        let limits = WatchLimits {
            read_timeout: Duration::from_millis(1),
            max_events: Some(3),
            exit_if_reopen_fails: true,
        };
        watch_events(
            || Ok(mock().push_report(&g1_report())),
            &mut decoder,
            &mut out,
            &limits,
        )
        .unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("key G1 down"), "{text}");
        assert!(text.contains("axis stick-x 127"), "{text}");
        assert!(text.contains("axis stick-y 127"), "{text}");
    }

    #[test]
    fn record_round_trips_through_the_decoder() {
        let descriptor = g13();
        let mut recorded = Cursor::new(Vec::new());
        record_reports(
            || Ok(mock().push_report(&g1_report()).push_disconnect()),
            &mut recorded,
            Some(1),
            Duration::from_millis(1),
            true,
        )
        .unwrap();
        let text = String::from_utf8(recorded.into_inner()).unwrap();
        let reports = parse_recorded_reports(&text).unwrap();
        assert_eq!(reports, [g1_report()]);
        let mut decoder = Decoder::from_descriptor(&descriptor).unwrap();
        let events = decoder.decode(&reports[0]).unwrap();
        assert!(events.contains(&InputEvent::KeyDown { name: "G1".into() }));
    }

    #[test]
    fn rgb_sends_feature_report_7_and_reads_it_back() {
        let mut transport = mock().push_feature_report(&[0x07, 0x10, 0x20, 0x30, 0x00]);
        let mut out = Vec::new();
        rgb(&mut transport, 0x10, 0x20, 0x30, &mut out).unwrap();
        assert_eq!(
            transport.feature_writes(),
            [vec![0x07, 0x10, 0x20, 0x30, 0x00]]
        );
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("readback"), "{text}");
    }

    #[test]
    fn leds_sends_feature_report_5() {
        let mut transport = mock();
        let mut out = Vec::new();
        leds(&mut transport, 0x05, &mut out).unwrap();
        assert_eq!(transport.feature_writes(), [vec![0x05, 0x05, 0, 0, 0]]);
    }

    #[test]
    fn lcd_test_sends_a_992_byte_report_3() {
        let mut transport = mock();
        let mut out = Vec::new();
        lcd_test(&mut transport, &mut out).unwrap();
        assert_eq!(transport.writes().len(), 1);
        let report = &transport.writes()[0];
        assert_eq!(report.len(), 992);
        assert_eq!(report[0], 0x03);
        assert!(report[1..=31].iter().all(|byte| *byte == 0));
        assert!(hidden_lcd_rows_are_zero(report));
    }

    #[test]
    fn no_cli_output_command_writes_feature_report_6() {
        let mut rgb_transport = mock().push_feature_report(&[0x07, 0, 0, 0, 0]);
        rgb(&mut rgb_transport, 1, 2, 3, &mut io::sink()).unwrap();
        let mut led_transport = mock();
        leds(&mut led_transport, 1, &mut io::sink()).unwrap();
        let mut lcd_transport = mock();
        lcd_test(&mut lcd_transport, &mut io::sink()).unwrap();

        for writes in [
            rgb_transport.feature_writes(),
            led_transport.feature_writes(),
            lcd_transport.feature_writes(),
        ] {
            assert!(
                writes.iter().all(|write| write.first() != Some(&0x06)),
                "{writes:?}"
            );
        }
        assert!(lcd_transport
            .writes()
            .iter()
            .all(|write| write.first() != Some(&0x06)));
    }

    #[test]
    fn watch_resumes_after_a_scripted_disconnect() {
        let descriptor = g13();
        let mut decoder = Decoder::from_descriptor(&descriptor).unwrap();
        let mut out = Vec::new();
        let limits = WatchLimits {
            read_timeout: Duration::from_millis(1),
            max_events: Some(3),
            exit_if_reopen_fails: true,
        };
        let started = Instant::now();
        let mut opens = 0u8;
        watch_events(
            || {
                opens += 1;
                match opens {
                    1 => Ok(mock().push_disconnect()),
                    _ => Ok(mock().push_report(&g1_report())),
                }
            },
            &mut decoder,
            &mut out,
            &limits,
        )
        .unwrap();
        let elapsed = started.elapsed();
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("reconnect"), "{text}");
        assert_eq!(text.matches("reconnect").count(), 1, "{text}");
        assert!(text.contains("key G1 down"), "{text}");
        assert!(elapsed <= Duration::from_secs(2), "{elapsed:?}");
    }

    #[test]
    fn parse_u8_accepts_hex_masks() {
        assert_eq!(parse_u8("0x0F").unwrap(), 0x0F);
        assert_eq!(parse_u8("15").unwrap(), 15);
        assert!(parse_u8("256").is_err());
    }
}
