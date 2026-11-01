//! `bosunctl` library surface so device commands can run over any [`Transport`].

mod listing;

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use bosun_hid::{
    format_recorded_report, hidden_lcd_rows_are_zero, lcd_test_report, leds_feature_report,
    load_descriptor, rgb_feature_report, Decoder, DeviceDescriptor, DeviceInfo, HidError,
    HidTransport, InputEvent, ReadOutcome, Transport, INPUT_REPORT_LEN, LCD_REPORT_LEN,
    RGB_REPORT_ID,
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
    #[arg(long, hide = true)]
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
    #[arg(long, hide = true)]
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

/// Interrupt reads use a buffer longer than any expected report, so an
/// oversized report shows up whole instead of being silently truncated.
const READ_BUFFER_LEN: usize = 64;

/// How `watch` and `record` ride out an unplug.
#[derive(Clone, Debug)]
pub struct ReconnectPolicy {
    /// Pause between attempts to reopen a device that went away.
    pub interval: Duration,
    /// Give up after this many consecutive failed attempts; `None` waits for
    /// a replug indefinitely.
    pub max_attempts: Option<usize>,
}

impl Default for ReconnectPolicy {
    fn default() -> Self {
        Self {
            // Polls enumeration four times a second: well inside AC-9's 2 s
            // budget without hammering the OS while the device is gone.
            interval: Duration::from_millis(250),
            max_attempts: None,
        }
    }
}

/// Limits for [`watch_events`] so tests do not block.
#[derive(Clone, Debug)]
pub struct WatchLimits {
    pub read_timeout: Duration,
    pub max_events: Option<usize>,
    pub reconnect: ReconnectPolicy,
}

impl Default for WatchLimits {
    fn default() -> Self {
        Self {
            read_timeout: Duration::from_millis(100),
            max_events: None,
            reconnect: ReconnectPolicy::default(),
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
                let written = record_reports(
                    || open_live(&descriptor, args.io.path.as_deref()),
                    &mut file,
                    args.max_reports,
                    Duration::from_millis(100),
                    &ReconnectPolicy::default(),
                )?;
                println!("recorded {written} reports to {}", args.output.display());
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

/// Read reports, decode them, and print events, riding out unplugs.
///
/// Any read error means the handle is dead: `hidapi` reports a physical
/// unplug as an opaque backend error on every OS, never as
/// [`HidError::Disconnected`]. Watch then prints one `disconnected` line,
/// releases held keys, reopens per `limits.reconnect`, and prints exactly one
/// `reconnect` line once the device is back.
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
    let mut printed = 0usize;
    let mut buf = [0u8; READ_BUFFER_LEN];

    loop {
        if limits.max_events.is_some_and(|max| printed >= max) {
            return Ok(());
        }

        let events = match transport.read(&mut buf, limits.read_timeout) {
            Ok(ReadOutcome::Timeout) => continue,
            Ok(ReadOutcome::Report(len)) => match decoder.decode(&buf[..len]) {
                Ok(events) => events,
                Err(error) => {
                    // Never drop a report silently: an unexpected shape is
                    // exactly what a hardware session needs to see.
                    writeln!(out, "report {} not decoded: {error}", hex(&buf[..len]))?;
                    continue;
                }
            },
            Err(error) => {
                writeln!(out, "disconnected: {error}")?;
                if print_events(out, &decoder.release_all(), &mut printed, limits.max_events)? {
                    return Ok(());
                }
                let reopened = reopen(&mut opener, &limits.reconnect, |reason| {
                    writeln!(out, "waiting: {reason}")
                })?;
                match reopened {
                    Some(next) => {
                        transport = next;
                        writeln!(out, "reconnect")?;
                        continue;
                    }
                    None => return Ok(()),
                }
            }
        };

        if print_events(out, &events, &mut printed, limits.max_events)? {
            return Ok(());
        }
    }
}

/// Print `events`, stopping at `max`. Returns true once the limit is reached.
fn print_events(
    out: &mut impl Write,
    events: &[InputEvent],
    printed: &mut usize,
    max: Option<usize>,
) -> io::Result<bool> {
    for event in events {
        writeln!(out, "{}", format_event(event))?;
        *printed += 1;
        if max.is_some_and(|max| *printed >= max) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Reopen a device whose handle died, waiting out an unplug.
///
/// `waiting` hears each distinct reopen failure once, so a long unplug costs
/// one line rather than one per attempt. Returns `None` after
/// `policy.max_attempts` consecutive failures.
fn reopen<T, F>(
    opener: &mut F,
    policy: &ReconnectPolicy,
    mut waiting: impl FnMut(&HidError) -> io::Result<()>,
) -> io::Result<Option<T>>
where
    F: FnMut() -> bosun_hid::Result<T>,
{
    let mut failures = 0usize;
    let mut last_reason: Option<String> = None;
    loop {
        match opener() {
            Ok(transport) => return Ok(Some(transport)),
            Err(error) => {
                let reason = error.to_string();
                if last_reason.as_deref() != Some(reason.as_str()) {
                    waiting(&error)?;
                    last_reason = Some(reason);
                }
                failures += 1;
                if policy.max_attempts.is_some_and(|max| failures >= max) {
                    return Ok(None);
                }
                std::thread::sleep(policy.interval);
            }
        }
    }
}

/// Write raw 8-byte reports as fixture lines, riding out unplugs.
///
/// Anything that is not a fixture line is written as a `#` comment, which
/// the fixture parser skips: unexpected report shapes, and the disconnect and
/// reconnect around an unplug. The capture stays parseable and keeps the
/// evidence.
pub fn record_reports<T, F, W>(
    mut opener: F,
    out: &mut W,
    max_reports: Option<usize>,
    read_timeout: Duration,
    reconnect: &ReconnectPolicy,
) -> Result<usize>
where
    T: Transport,
    F: FnMut() -> bosun_hid::Result<T>,
    W: Write,
{
    let mut transport = opener().context("could not open the device")?;
    let mut written = 0usize;
    let mut buf = [0u8; READ_BUFFER_LEN];

    loop {
        if max_reports.is_some_and(|max| written >= max) {
            return Ok(written);
        }

        match transport.read(&mut buf, read_timeout) {
            Ok(ReadOutcome::Timeout) => {}
            Ok(ReadOutcome::Report(len)) => {
                let report = &buf[..len];
                match <[u8; INPUT_REPORT_LEN]>::try_from(report) {
                    Ok(fixture) => {
                        writeln!(out, "{}", format_recorded_report(&fixture))?;
                        written += 1;
                    }
                    Err(_) => writeln!(out, "# unexpected {len}-byte report: {}", hex(report))?,
                }
            }
            Err(error) => {
                writeln!(out, "# disconnected: {error}")?;
                let reopened = reopen(&mut opener, reconnect, |reason| {
                    writeln!(out, "# waiting: {reason}")
                })?;
                match reopened {
                    Some(next) => {
                        transport = next;
                        writeln!(out, "# reconnect")?;
                    }
                    None => return Ok(written),
                }
            }
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
    debug_assert_eq!(report.len(), LCD_REPORT_LEN);
    debug_assert_eq!(report[0], 0x03);
    debug_assert!(report[1..=31].iter().all(|byte| *byte == 0));
    debug_assert!(hidden_lcd_rows_are_zero(&report));
    let written = transport
        .write(&report)
        .context("could not write LCD test report")?;
    writeln!(out, "lcd test wrote {written} bytes report={}", report[0])?;
    Ok(())
}

/// Lowercase space-separated hex, the same shape as a recorded fixture line.
pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
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

    fn rest_report() -> [u8; 8] {
        [0x01, 0x7F, 0x7F, 0, 0, 0, 0, 0]
    }

    /// What hidapi's Linux backend says when the cable is pulled mid-read.
    const UNPLUG: &str = "hid_read_timeout: unexpected poll error (device disconnected)";

    /// Reopen without sleeping and give up after `attempts` failures, so no
    /// test sleeps or hangs.
    fn quick_reconnect(attempts: usize) -> ReconnectPolicy {
        ReconnectPolicy {
            interval: Duration::ZERO,
            max_attempts: Some(attempts),
        }
    }

    fn limits(max_events: usize) -> WatchLimits {
        WatchLimits {
            read_timeout: Duration::from_millis(1),
            max_events: Some(max_events),
            reconnect: quick_reconnect(1),
        }
    }

    fn list_args<const N: usize>(argv: [&str; N]) -> ListArgs {
        match Cli::parse_from(argv).command {
            Command::Device {
                command: DeviceCommand::List(args),
            } => args,
            other => panic!("{other:?}"),
        }
    }

    fn g13_vendor_collection() -> DeviceInfo {
        DeviceInfo {
            path: "vendor".to_owned(),
            vendor_id: 0x046D,
            product_id: 0xC21C,
            usage_page: 0xFF00,
            ..DeviceInfo::default()
        }
    }

    fn no_report_six(writes: &[Vec<u8>]) {
        assert!(
            writes.iter().all(|write| write.first() != Some(&0x06)),
            "{writes:?}"
        );
    }

    #[test]
    fn the_cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn identifiers_accept_hex_on_the_command_line() {
        let args = list_args([
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
        assert_eq!(args.vid, Some(0x046D));
        assert_eq!(args.pid, Some(0xC21C));
        assert_eq!(args.usage_page, Some(0xFF00));
    }

    #[test]
    fn an_unfiltered_list_selects_everything() {
        let args = list_args(["bosunctl", "device", "list"]);
        assert!(args.selects(&g13_vendor_collection()));
        assert!(args.selects(&DeviceInfo::default()));
    }

    #[test]
    fn new_device_commands_parse() {
        assert!(matches!(
            Cli::parse_from(["bosunctl", "device", "info"]).command,
            Command::Device {
                command: DeviceCommand::Info(_)
            }
        ));
        assert!(matches!(
            Cli::parse_from(["bosunctl", "device", "watch"]).command,
            Command::Device {
                command: DeviceCommand::Watch(_)
            }
        ));
        assert!(matches!(
            Cli::parse_from(["bosunctl", "device", "record", "--output", "out.hex"]).command,
            Command::Device {
                command: DeviceCommand::Record(_)
            }
        ));
        assert!(matches!(
            Cli::parse_from(["bosunctl", "device", "rgb", "1", "2", "3"]).command,
            Command::Device {
                command: DeviceCommand::Rgb(_)
            }
        ));
        assert!(matches!(
            Cli::parse_from(["bosunctl", "device", "leds", "0x0F"]).command,
            Command::Device {
                command: DeviceCommand::Leds(_)
            }
        ));
        assert!(matches!(
            Cli::parse_from(["bosunctl", "device", "lcd", "test"]).command,
            Command::Device {
                command: DeviceCommand::Lcd {
                    command: LcdCommand::Test(_)
                }
            }
        ));
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
        watch_events(
            || Ok(mock().push_report(&g1_report())),
            &mut decoder,
            &mut out,
            &limits(3),
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
            &quick_reconnect(1),
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

        no_report_six(rgb_transport.feature_writes());
        no_report_six(led_transport.feature_writes());
        no_report_six(lcd_transport.feature_writes());
        no_report_six(lcd_transport.writes());

        let src = include_str!("lib.rs");
        let watch = src.find("pub fn watch_events").expect("watch_events");
        let record = src.find("pub fn record_reports").expect("record_reports");
        let rgb_fn = src.find("pub fn rgb<").expect("rgb");
        assert!(
            !src[watch..record].contains("send_feature_report"),
            "watch must not send feature reports"
        );
        assert!(
            !src[record..rgb_fn].contains("send_feature_report"),
            "record must not send feature reports"
        );
    }

    #[test]
    fn watch_resumes_after_a_scripted_disconnect() {
        let descriptor = g13();
        let mut decoder = Decoder::from_descriptor(&descriptor).unwrap();
        let mut out = Vec::new();
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
            &limits(3),
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
    fn watch_reconnects_when_a_read_fails_the_way_a_real_unplug_does() {
        // Regression: real backends never return `Disconnected`, so watch
        // used to exit with an error on a physical unplug (AC-9).
        let mut decoder = Decoder::from_descriptor(&g13()).unwrap();
        let mut out = Vec::new();
        let mut opens = 0u8;
        watch_events(
            || {
                opens += 1;
                match opens {
                    1 => Ok(mock().push_error(UNPLUG)),
                    _ => Ok(mock().push_report(&g1_report())),
                }
            },
            &mut decoder,
            &mut out,
            &limits(3),
        )
        .unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(
            text.contains(&format!("disconnected: HID backend failure: {UNPLUG}")),
            "{text}"
        );
        assert_eq!(text.matches("reconnect").count(), 1, "{text}");
        assert!(text.contains("key G1 down"), "{text}");
    }

    #[test]
    fn watch_keeps_reopening_until_the_device_returns_then_reconnects_once() {
        let mut decoder = Decoder::from_descriptor(&g13()).unwrap();
        let mut out = Vec::new();
        let absent = HidError::NotFound(g13().match_criteria.device_match());
        let mut opens = 0u8;
        let limits = WatchLimits {
            reconnect: quick_reconnect(10),
            ..limits(3)
        };
        watch_events(
            || {
                opens += 1;
                match opens {
                    1 => Ok(mock().push_error(UNPLUG)),
                    2..=4 => Err(HidError::NotFound(g13().match_criteria.device_match())),
                    _ => Ok(mock().push_report(&g1_report())),
                }
            },
            &mut decoder,
            &mut out,
            &limits,
        )
        .unwrap();
        assert_eq!(opens, 5);
        let text = String::from_utf8(out).unwrap();
        assert_eq!(text.matches("reconnect").count(), 1, "{text}");
        // Why the device cannot be reopened is said once, not per attempt.
        assert_eq!(
            text.matches(&format!("waiting: {absent}")).count(),
            1,
            "{text}"
        );
        assert!(text.contains("key G1 down"), "{text}");
    }

    #[test]
    fn watch_stops_after_the_configured_number_of_failed_reopens() {
        let mut decoder = Decoder::from_descriptor(&g13()).unwrap();
        let mut out = Vec::new();
        let criteria = g13().match_criteria.device_match();
        let mut opens = 0u8;
        let limits = WatchLimits {
            reconnect: quick_reconnect(3),
            ..limits(10)
        };
        watch_events(
            || {
                opens += 1;
                if opens == 1 {
                    Ok(mock().push_error(UNPLUG))
                } else {
                    Err(HidError::NotFound(criteria))
                }
            },
            &mut decoder,
            &mut out,
            &limits,
        )
        .unwrap();
        assert_eq!(opens, 4);
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("disconnected: "), "{text}");
        assert!(!text.contains("reconnect"), "{text}");
    }

    #[test]
    fn watch_releases_held_keys_when_the_device_disappears() {
        let mut decoder = Decoder::from_descriptor(&g13()).unwrap();
        let mut out = Vec::new();
        let mut opens = 0u8;
        watch_events(
            || {
                opens += 1;
                match opens {
                    1 => Ok(mock().push_report(&g1_report()).push_error(UNPLUG)),
                    _ => Ok(mock().push_report(&rest_report())),
                }
            },
            &mut decoder,
            &mut out,
            &limits(6),
        )
        .unwrap();
        let text = String::from_utf8(out).unwrap();
        let disconnected = format!("disconnected: HID backend failure: {UNPLUG}");
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(
            lines,
            [
                "key G1 down",
                "axis stick-x 127",
                "axis stick-y 127",
                disconnected.as_str(),
                // Nothing is held on a device that is gone.
                "key G1 up",
                "reconnect",
                // A fresh edge stream: the stick reports where it is.
                "axis stick-x 127",
                "axis stick-y 127",
            ]
        );
    }

    #[test]
    fn watch_prints_reports_it_cannot_decode_instead_of_dropping_them() {
        let mut decoder = Decoder::from_descriptor(&g13()).unwrap();
        let mut out = Vec::new();
        watch_events(
            || {
                Ok(mock()
                    .push_report(&[0x02, 0xAA, 0xBB])
                    .push_report(&g1_report()))
            },
            &mut decoder,
            &mut out,
            &limits(1),
        )
        .unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(
            text.contains("report 02 aa bb not decoded: input report must be 8 bytes, got 3"),
            "{text}"
        );
        assert!(text.contains("key G1 down"), "{text}");
    }

    #[test]
    fn record_keeps_unexpected_reports_as_comments() {
        let mut recorded = Vec::new();
        let written = record_reports(
            || Ok(mock().push_report(&[0x02, 0xAA]).push_report(&g1_report())),
            &mut recorded,
            Some(1),
            Duration::from_millis(1),
            &quick_reconnect(1),
        )
        .unwrap();
        assert_eq!(written, 1);
        let text = String::from_utf8(recorded).unwrap();
        assert!(text.contains("# unexpected 2-byte report: 02 aa"), "{text}");
        // Comments keep the capture a valid fixture.
        assert_eq!(parse_recorded_reports(&text).unwrap(), [g1_report()]);
    }

    #[test]
    fn record_rides_out_an_unplug_and_marks_the_gap() {
        let mut recorded = Vec::new();
        let mut opens = 0u8;
        let written = record_reports(
            || {
                opens += 1;
                match opens {
                    1 => Ok(mock().push_report(&g1_report()).push_error(UNPLUG)),
                    _ => Ok(mock().push_report(&rest_report())),
                }
            },
            &mut recorded,
            Some(2),
            Duration::from_millis(1),
            &quick_reconnect(1),
        )
        .unwrap();
        assert_eq!(written, 2);
        let text = String::from_utf8(recorded).unwrap();
        assert!(
            text.contains(&format!("# disconnected: HID backend failure: {UNPLUG}")),
            "{text}"
        );
        assert!(text.contains("# reconnect"), "{text}");
        assert_eq!(
            parse_recorded_reports(&text).unwrap(),
            [g1_report(), rest_report()]
        );
    }

    #[test]
    fn parse_u8_accepts_hex_masks() {
        assert_eq!(parse_u8("0x0F").unwrap(), 0x0F);
        assert_eq!(parse_u8("15").unwrap(), 15);
        assert!(parse_u8("256").is_err());
    }
}
