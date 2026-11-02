//! `bosunctl device probe`: one read-only run that makes an R1 hardware
//! session decisive.
//!
//! AC-R1 measured "fail both" on 2026-09-20 from a single signal: zero
//! interrupt reports in 15 s. That signal cannot tell "a second reader is
//! starved" from "no key was pressed in the window". The probe collects
//! independent evidence from the same handle:
//!
//! - every collection the descriptor's VID/PID exposes, so a sibling on
//!   another path or usage page shows up;
//! - the report descriptor the OS hands back for the opened interface;
//! - a feature-report readback, proving the control pipe answers;
//! - during the window, interrupt reads interleaved with HID `GET_REPORT`
//!   polls of the input report. If polling sees keys or the stick change
//!   while the interrupt stream stays silent, keys were pressed and the
//!   device reports them, so the shared interrupt read is what fails.
//!
//! Nothing here sends an output or feature report.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::time::Duration;

use anyhow::Result;
use bosun_hid::{
    select_index, CodecError, Decoder, DeviceDescriptor, DeviceInfo, DeviceMatch, InputEvent,
    ReadOutcome, Transport, INPUT_REPORT_ID, MAX_REPORT_DESCRIPTOR_LEN,
};

use crate::listing::format_device;
use crate::{format_event, hex, read_feature_report, READ_BUFFER_LEN};

/// Raw lines printed per channel before the probe only counts.
const RAW_LINE_LIMIT: usize = 24;
/// Polling stops after this many `GET_REPORT` failures in a row.
const MAX_CONSECUTIVE_POLL_FAILURES: usize = 3;
/// A progress line this often, so a silent window still looks alive.
const HEARTBEAT: Duration = Duration::from_secs(5);
/// Feature readbacks print this many bytes before eliding the rest.
const FEATURE_PREVIEW_LEN: usize = 16;

/// How long and how often to look for input.
#[derive(Clone, Debug)]
pub struct ProbeOptions {
    /// Total time to watch while the operator presses keys.
    pub window: Duration,
    /// Interrupt read timeout. One `GET_REPORT` poll follows each read.
    pub poll_interval: Duration,
    /// Poll the input report over `GET_REPORT`. Switching it off is the
    /// escape hatch for a device that blocks on that request.
    pub poll_input_report: bool,
}

/// What one probe window saw on each channel.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InputEvidence {
    /// Interrupt reports read, keyed by (length, first byte).
    pub interrupt_shapes: BTreeMap<(usize, u8), usize>,
    /// Interrupt reports that decoded against the descriptor.
    pub interrupt_decoded: usize,
    /// Keys seen pressed on the interrupt stream.
    pub interrupt_keys: BTreeSet<String>,
    /// `GET_REPORT` polls that answered.
    pub polls_ok: usize,
    /// `GET_REPORT` polls that failed.
    pub polls_failed: usize,
    /// The first `GET_REPORT` failure, verbatim.
    pub first_poll_error: Option<String>,
    /// Times a polled report differed from the one before it.
    pub polled_changes: usize,
    /// Keys seen pressed in polled reports.
    pub polled_keys: BTreeSet<String>,
    /// Lowest and highest value seen per axis, across both channels.
    pub axis_ranges: BTreeMap<String, (u8, u8)>,
    /// The interrupt read failure that ended the window early.
    pub read_error: Option<String>,
}

impl InputEvidence {
    /// Interrupt reports of any shape.
    pub fn interrupt_reports(&self) -> usize {
        self.interrupt_shapes.values().sum()
    }

    /// The single conclusion this evidence supports.
    pub fn verdict(&self) -> Verdict {
        if self.interrupt_decoded > 0 {
            Verdict::InterruptLive
        } else if self.interrupt_reports() > 0 {
            Verdict::InterruptUndecoded
        } else if self.polled_changes > 0 || !self.polled_keys.is_empty() {
            Verdict::PolledOnly
        } else if self.polls_ok > 0 {
            Verdict::PolledStatic
        } else {
            Verdict::Silent
        }
    }
}

/// The conclusion of one probe window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Decodable input reports reached this reader on the interrupt stream.
    InterruptLive,
    /// Interrupt reports arrived, but none matched the descriptor.
    InterruptUndecoded,
    /// The interrupt stream was silent while polled state changed.
    PolledOnly,
    /// The interrupt stream was silent and polled state never changed.
    PolledStatic,
    /// Nothing on either channel.
    Silent,
}

impl Verdict {
    /// Stable token for run logs and `docs/hardware-notes.md`.
    pub fn token(self) -> &'static str {
        match self {
            Self::InterruptLive => "interrupt-live",
            Self::InterruptUndecoded => "interrupt-undecoded",
            Self::PolledOnly => "polled-only",
            Self::PolledStatic => "polled-static",
            Self::Silent => "silent",
        }
    }

    /// What the verdict means and what to do next.
    pub fn meaning(self) -> &'static str {
        match self {
            Self::InterruptLive => {
                "Input reports reach this reader. For AC-R1, record a pass for the LGS state \
                 of this run."
            }
            Self::InterruptUndecoded => {
                "Reports arrive on the interrupt stream, but none is the descriptor's 8-byte \
                 report 0x01. Compare the raw lines with docs/BOSUN-PLAN.md §2.4 before \
                 changing anything."
            }
            Self::PolledOnly => {
                "GET_REPORT returned live key or stick state while nothing arrived on the \
                 interrupt stream. Keys were pressed and the device reports them, so the \
                 shared interrupt read is what fails. Record this result. Reading input by \
                 polling would be a plan change for Kyle."
            }
            Self::PolledStatic => {
                "GET_REPORT answers, but its state never changed, so there is no evidence \
                 that a key was pressed. Rerun while pressing keys and moving the stick."
            }
            Self::Silent => {
                "No input on either channel. If LGS is running, quit it (LCore.exe) and \
                 rerun before any redesign (AC-R1)."
            }
        }
    }
}

/// Print every enumerated interface sharing the descriptor's VID/PID, and
/// mark the one its full match (VID, PID, usage page) selects.
pub fn print_inventory(
    devices: &[DeviceInfo],
    criteria: &DeviceMatch,
    out: &mut impl Write,
) -> Result<()> {
    let selected = select_index(devices, criteria);
    let siblings: Vec<(usize, &DeviceInfo)> = devices
        .iter()
        .enumerate()
        .filter(|(_, info)| {
            info.vendor_id == criteria.vendor_id && info.product_id == criteria.product_id
        })
        .collect();

    writeln!(
        out,
        "interfaces with vid:pid {:04x}:{:04x}: {}",
        criteria.vendor_id,
        criteria.product_id,
        siblings.len()
    )?;
    for (index, info) in siblings {
        let role = if Some(index) == selected {
            "selected"
        } else if criteria.matches(info) {
            "also matches"
        } else {
            "other collection"
        };
        writeln!(out, "  [{role}] {}", format_device(info))?;
    }
    if selected.is_none() {
        writeln!(out, "  no interface matches {criteria}")?;
    }
    Ok(())
}

/// Probe an opened interface without writing to it, printing as it goes.
///
/// `now` is the time since any fixed origin. It is read once before the
/// window and exactly once per pass, so a test clock that advances a fixed
/// step per reading drives a fixed number of passes.
pub fn probe<T: Transport>(
    transport: &mut T,
    descriptor: &DeviceDescriptor,
    options: &ProbeOptions,
    now: impl FnMut() -> Duration,
    out: &mut impl Write,
) -> Result<InputEvidence> {
    writeln!(out, "opened {}", format_device(transport.info()))?;
    print_report_descriptor(transport, out)?;
    print_feature_readback(transport, descriptor.caps.rgb.report_id, "rgb", out)?;

    let evidence = watch_input(transport, descriptor, options, now, out)?;
    print_summary(&evidence, options, out)?;
    Ok(evidence)
}

fn print_report_descriptor<T: Transport>(transport: &mut T, out: &mut impl Write) -> Result<()> {
    let mut buf = vec![0u8; MAX_REPORT_DESCRIPTOR_LEN];
    match transport.get_report_descriptor(&mut buf) {
        Ok(len) => {
            writeln!(out, "report descriptor: {len} bytes")?;
            for line in buf[..len].chunks(16) {
                writeln!(out, "  {}", hex(line))?;
            }
        }
        Err(error) => writeln!(out, "report descriptor: unavailable ({error})")?,
    }
    Ok(())
}

fn print_feature_readback<T: Transport>(
    transport: &mut T,
    report_id: u8,
    label: &str,
    out: &mut impl Write,
) -> Result<()> {
    match read_feature_report(transport, report_id) {
        Ok(report) => writeln!(
            out,
            "feature report {report_id} ({label}) readback: {}",
            hex_preview(&report, FEATURE_PREVIEW_LEN)
        )?,
        Err(error) => writeln!(
            out,
            "feature report {report_id} ({label}) readback failed: {error}"
        )?,
    }
    Ok(())
}

/// Interleave interrupt reads with `GET_REPORT` polls for `options.window`.
fn watch_input<T: Transport>(
    transport: &mut T,
    descriptor: &DeviceDescriptor,
    options: &ProbeOptions,
    mut now: impl FnMut() -> Duration,
    out: &mut impl Write,
) -> Result<InputEvidence> {
    // One decoder per channel: each channel's edges are diffed against its
    // own previous report.
    let mut interrupt_decoder = Decoder::from_descriptor(descriptor)?;
    let mut polled_decoder = Decoder::from_descriptor(descriptor)?;
    let mut evidence = InputEvidence::default();
    let mut polling = options.poll_input_report;
    let mut consecutive_poll_failures = 0usize;
    let mut last_polled: Option<Vec<u8>> = None;
    let mut interrupt_lines = 0usize;
    let mut polled_lines = 0usize;
    let mut next_heartbeat = HEARTBEAT;
    let mut buf = [0u8; READ_BUFFER_LEN];

    writeln!(
        out,
        "watching for {:.1}s: press and release several keys and move the stick now",
        options.window.as_secs_f64()
    )?;
    let started = now();
    loop {
        let elapsed = now().saturating_sub(started);
        if elapsed >= options.window {
            break;
        }
        let at = elapsed.as_secs_f64();
        if elapsed >= next_heartbeat {
            writeln!(
                out,
                "  {at:>7.3}s  {} interrupt reports, {} polls answered so far",
                evidence.interrupt_reports(),
                evidence.polls_ok
            )?;
            next_heartbeat += HEARTBEAT;
        }

        match transport.read(&mut buf, options.poll_interval) {
            Ok(ReadOutcome::Timeout) => {}
            Ok(ReadOutcome::Report(len)) => {
                let report = &buf[..len];
                let shape = (len, report.first().copied().unwrap_or_default());
                *evidence.interrupt_shapes.entry(shape).or_default() += 1;
                let decoded = interrupt_decoder.decode(report);
                if decoded.is_ok() {
                    evidence.interrupt_decoded += 1;
                }
                let note = note_events(
                    decoded,
                    &mut evidence.interrupt_keys,
                    &mut evidence.axis_ranges,
                );
                if interrupt_lines < RAW_LINE_LIMIT {
                    writeln!(out, "  {at:>7.3}s  interrupt  {}  {note}", hex(report))?;
                    interrupt_lines += 1;
                }
            }
            Err(error) => {
                writeln!(
                    out,
                    "  {at:>7.3}s  interrupt read failed, ending the window: {error}"
                )?;
                evidence.read_error = Some(error.to_string());
                break;
            }
        }

        if !polling {
            continue;
        }
        let mut poll = [0u8; READ_BUFFER_LEN];
        poll[0] = INPUT_REPORT_ID;
        match transport.get_input_report(&mut poll) {
            Ok(len) => {
                consecutive_poll_failures = 0;
                evidence.polls_ok += 1;
                let report = &poll[..len];
                if last_polled.as_deref() == Some(report) {
                    continue;
                }
                if last_polled.is_some() {
                    evidence.polled_changes += 1;
                }
                let note = note_events(
                    polled_decoder.decode(report),
                    &mut evidence.polled_keys,
                    &mut evidence.axis_ranges,
                );
                if polled_lines < RAW_LINE_LIMIT {
                    writeln!(out, "  {at:>7.3}s  polled     {}  {note}", hex(report))?;
                    polled_lines += 1;
                }
                last_polled = Some(report.to_vec());
            }
            Err(error) => {
                evidence.polls_failed += 1;
                consecutive_poll_failures += 1;
                if evidence.first_poll_error.is_none() {
                    evidence.first_poll_error = Some(error.to_string());
                }
                if consecutive_poll_failures >= MAX_CONSECUTIVE_POLL_FAILURES {
                    writeln!(
                        out,
                        "  {at:>7.3}s  polling stopped after {consecutive_poll_failures} \
                         failures in a row: {error}"
                    )?;
                    polling = false;
                }
            }
        }
    }
    Ok(evidence)
}

/// Describe a decode result for a raw line, collecting pressed keys and axis
/// ranges along the way.
fn note_events(
    decoded: std::result::Result<Vec<InputEvent>, CodecError>,
    keys: &mut BTreeSet<String>,
    axis_ranges: &mut BTreeMap<String, (u8, u8)>,
) -> String {
    let events = match decoded {
        Ok(events) => events,
        Err(error) => return format!("(not decoded: {error})"),
    };
    for event in &events {
        match event {
            InputEvent::KeyDown { name } => {
                keys.insert(name.clone());
            }
            InputEvent::KeyUp { .. } => {}
            InputEvent::Axis { name, value } => {
                let range = axis_ranges.entry(name.clone()).or_insert((*value, *value));
                range.0 = range.0.min(*value);
                range.1 = range.1.max(*value);
            }
        }
    }
    events
        .iter()
        .map(format_event)
        .collect::<Vec<_>>()
        .join(", ")
}

fn print_summary(
    evidence: &InputEvidence,
    options: &ProbeOptions,
    out: &mut impl Write,
) -> Result<()> {
    let shapes = if evidence.interrupt_shapes.is_empty() {
        "none".to_owned()
    } else {
        evidence
            .interrupt_shapes
            .iter()
            .map(|((len, id), count)| format!("{count} x {len} bytes id 0x{id:02x}"))
            .collect::<Vec<_>>()
            .join(", ")
    };

    writeln!(out, "summary")?;
    writeln!(
        out,
        "  interrupt reports: {} ({} decoded; shapes: {shapes})",
        evidence.interrupt_reports(),
        evidence.interrupt_decoded
    )?;
    if options.poll_input_report {
        writeln!(
            out,
            "  get-input-report polls: {} answered, {} failed, {} state changes",
            evidence.polls_ok, evidence.polls_failed, evidence.polled_changes
        )?;
        if let Some(error) = &evidence.first_poll_error {
            writeln!(out, "  first poll failure: {error}")?;
        }
    } else {
        writeln!(out, "  get-input-report polls: off")?;
    }
    writeln!(
        out,
        "  keys via interrupt: {}",
        names(&evidence.interrupt_keys)
    )?;
    writeln!(out, "  keys via polling: {}", names(&evidence.polled_keys))?;
    writeln!(out, "  axis ranges: {}", ranges(&evidence.axis_ranges))?;
    if let Some(error) = &evidence.read_error {
        writeln!(out, "  window ended early: {error}")?;
    }

    let verdict = evidence.verdict();
    writeln!(out, "verdict: {}", verdict.token())?;
    writeln!(out, "  {}", verdict.meaning())?;
    Ok(())
}

fn names(keys: &BTreeSet<String>) -> String {
    if keys.is_empty() {
        return "none".to_owned();
    }
    keys.iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(" ")
}

fn ranges(axes: &BTreeMap<String, (u8, u8)>) -> String {
    if axes.is_empty() {
        return "none".to_owned();
    }
    axes.iter()
        .map(|(name, (low, high))| format!("{name} {low}..{high}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Hex of the first `limit` bytes, noting the full length when cut short.
fn hex_preview(bytes: &[u8], limit: usize) -> String {
    if bytes.len() <= limit {
        return hex(bytes);
    }
    format!("{} .. ({} bytes)", hex(&bytes[..limit]), bytes.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::PathBuf;

    use bosun_hid::{load_descriptor, MockTransport};

    const STEP: Duration = Duration::from_millis(10);
    const UNPLUG: &str = "hid_read_timeout: unexpected poll error (device disconnected)";

    fn g13() -> DeviceDescriptor {
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../devices/logitech-g13.toml");
        load_descriptor(path).unwrap()
    }

    fn mock() -> MockTransport {
        MockTransport::new(DeviceInfo {
            path: "mock".into(),
            vendor_id: 0x046D,
            product_id: 0xC21C,
            usage_page: 0xFF00,
            ..DeviceInfo::default()
        })
    }

    fn rest() -> [u8; 8] {
        [0x01, 0x7F, 0x7F, 0, 0, 0, 0, 0]
    }

    fn g1() -> [u8; 8] {
        [0x01, 0x7F, 0x7F, 0x01, 0, 0, 0, 0]
    }

    /// A clock that advances one `STEP` per reading.
    fn ticking() -> impl FnMut() -> Duration {
        let mut ticks = 0u32;
        move || {
            let now = STEP * ticks;
            ticks += 1;
            now
        }
    }

    /// Options under which `ticking` drives exactly `iterations` passes: the
    /// probe reads the clock once before the window and once per pass.
    fn options(iterations: u32) -> ProbeOptions {
        ProbeOptions {
            window: STEP * (iterations + 1),
            poll_interval: STEP,
            poll_input_report: true,
        }
    }

    fn probe_mock(
        transport: &mut MockTransport,
        options: &ProbeOptions,
    ) -> (InputEvidence, String) {
        let mut out = Vec::new();
        let evidence = probe(transport, &g13(), options, ticking(), &mut out).unwrap();
        (evidence, String::from_utf8(out).unwrap())
    }

    #[test]
    fn interrupt_reports_mean_shared_reads_work() {
        let mut transport = mock()
            .push_report(&rest())
            .push_report(&g1())
            .push_input_report(&rest())
            .push_input_report(&rest());

        let (evidence, text) = probe_mock(&mut transport, &options(2));

        assert_eq!(evidence.verdict(), Verdict::InterruptLive, "{text}");
        assert_eq!(evidence.interrupt_decoded, 2);
        assert_eq!(evidence.interrupt_shapes, BTreeMap::from([((8, 0x01), 2)]));
        assert_eq!(evidence.interrupt_keys, BTreeSet::from(["G1".to_owned()]));
        assert!(evidence.read_error.is_none(), "{text}");
        assert!(
            text.contains("interrupt  01 7f 7f 01 00 00 00 00  key G1 down"),
            "{text}"
        );
        assert!(text.contains("verdict: interrupt-live"), "{text}");
    }

    #[test]
    fn a_silent_interrupt_stream_with_live_polls_points_at_the_interrupt_path() {
        let mut stick_east = rest();
        stick_east[1] = 0xFF;
        let mut transport = mock()
            .push_timeout()
            .push_timeout()
            .push_timeout()
            .push_input_report(&rest())
            .push_input_report(&g1())
            .push_input_report(&stick_east);

        let (evidence, text) = probe_mock(&mut transport, &options(3));

        assert_eq!(evidence.verdict(), Verdict::PolledOnly, "{text}");
        assert_eq!(evidence.interrupt_reports(), 0);
        assert_eq!(evidence.polls_ok, 3);
        assert_eq!(evidence.polled_changes, 2);
        assert_eq!(evidence.polled_keys, BTreeSet::from(["G1".to_owned()]));
        assert_eq!(evidence.axis_ranges["stick-x"], (0x7F, 0xFF));
        assert_eq!(evidence.axis_ranges["stick-y"], (0x7F, 0x7F));
        assert!(
            text.contains("polled     01 7f 7f 01 00 00 00 00  key G1 down"),
            "{text}"
        );
        assert!(text.contains("verdict: polled-only"), "{text}");
    }

    #[test]
    fn unchanging_polls_are_no_evidence_of_a_keypress() {
        let mut transport = mock()
            .push_timeout()
            .push_timeout()
            .push_input_report(&rest())
            .push_input_report(&rest());

        let (evidence, text) = probe_mock(&mut transport, &options(2));

        assert_eq!(evidence.verdict(), Verdict::PolledStatic, "{text}");
        assert_eq!(evidence.polls_ok, 2);
        assert_eq!(evidence.polled_changes, 0);
        // An unchanged poll is not reprinted.
        assert_eq!(text.matches("polled     ").count(), 1, "{text}");
        assert!(text.contains("verdict: polled-static"), "{text}");
    }

    #[test]
    fn polling_stops_after_repeated_failures_and_seeing_nothing_is_silent() {
        let mut transport = mock()
            .push_timeout()
            .push_timeout()
            .push_timeout()
            .push_timeout()
            .push_timeout()
            .push_input_report_error("GET_REPORT stalled")
            .push_input_report_error("GET_REPORT stalled")
            .push_input_report_error("GET_REPORT stalled")
            .push_input_report(&g1());

        let (evidence, text) = probe_mock(&mut transport, &options(5));

        assert_eq!(evidence.polls_failed, 3);
        // The answer queued after the failures is never asked for.
        assert_eq!(evidence.polls_ok, 0);
        assert_eq!(
            evidence.first_poll_error.as_deref(),
            Some("HID backend failure: GET_REPORT stalled")
        );
        assert_eq!(evidence.verdict(), Verdict::Silent, "{text}");
        assert!(
            text.contains("polling stopped after 3 failures in a row"),
            "{text}"
        );
        assert!(text.contains("verdict: silent"), "{text}");
    }

    #[test]
    fn interrupt_reports_of_an_unexpected_shape_are_shown_not_dropped() {
        let mut transport = mock()
            .push_report(&[0x02, 0xAA, 0xBB])
            .push_input_report(&rest());

        let (evidence, text) = probe_mock(&mut transport, &options(1));

        assert_eq!(evidence.verdict(), Verdict::InterruptUndecoded, "{text}");
        assert_eq!(evidence.interrupt_shapes, BTreeMap::from([((3, 0x02), 1)]));
        assert!(
            text.contains(
                "interrupt  02 aa bb  (not decoded: input report must be 8 bytes, got 3)"
            ),
            "{text}"
        );
        assert!(text.contains("3 bytes id 0x02"), "{text}");
        assert!(text.contains("verdict: interrupt-undecoded"), "{text}");
    }

    #[test]
    fn a_lost_device_ends_the_window_early_and_says_why() {
        let mut transport = mock().push_error(UNPLUG).push_timeout();

        let (evidence, text) = probe_mock(&mut transport, &options(5));

        let expected = format!("HID backend failure: {UNPLUG}");
        assert_eq!(evidence.read_error.as_deref(), Some(expected.as_str()));
        assert_eq!(
            evidence.polls_ok + evidence.polls_failed,
            0,
            "no poll after the read failed"
        );
        assert!(
            text.contains("interrupt read failed, ending the window"),
            "{text}"
        );
        assert!(
            text.contains(&format!("window ended early: {expected}")),
            "{text}"
        );
    }

    #[test]
    fn polling_can_be_switched_off() {
        let mut transport = mock().push_timeout().push_input_report(&g1());
        let options = ProbeOptions {
            poll_input_report: false,
            ..options(1)
        };

        let (evidence, text) = probe_mock(&mut transport, &options);

        assert_eq!(evidence.polls_ok + evidence.polls_failed, 0);
        assert!(text.contains("get-input-report polls: off"), "{text}");
        assert_eq!(evidence.verdict(), Verdict::Silent);
    }

    #[test]
    fn the_report_descriptor_and_a_feature_readback_are_printed() {
        let descriptor_bytes: Vec<u8> = (0u8..20).collect();
        let mut transport = mock()
            .with_report_descriptor(&descriptor_bytes)
            .push_feature_report(&[0x07, 0x6E, 0xFF, 0x5A, 0x00])
            .push_timeout()
            .push_input_report(&rest());

        let (_, text) = probe_mock(&mut transport, &options(1));

        assert!(text.contains("report descriptor: 20 bytes"), "{text}");
        assert!(
            text.contains("  00 01 02 03 04 05 06 07 08 09 0a 0b 0c 0d 0e 0f\n  10 11 12 13\n"),
            "{text}"
        );
        assert!(
            text.contains("feature report 7 (rgb) readback: 07 6e ff 5a 00"),
            "{text}"
        );
    }

    #[test]
    fn a_padded_feature_readback_is_previewed_with_its_full_length() {
        let mut padded = vec![0u8; 258];
        padded[..5].copy_from_slice(&[0x07, 0x6E, 0xFF, 0x5A, 0x00]);
        let mut transport = mock()
            .push_feature_report(&padded)
            .push_timeout()
            .push_input_report(&rest());

        let (_, text) = probe_mock(&mut transport, &options(1));

        assert!(
            text.contains(
                "feature report 7 (rgb) readback: \
                 07 6e ff 5a 00 00 00 00 00 00 00 00 00 00 00 00 .. (258 bytes)"
            ),
            "{text}"
        );
    }

    #[test]
    fn missing_descriptor_and_feature_support_are_reported_not_fatal() {
        let mut transport = mock().push_timeout().push_input_report(&rest());

        let (_, text) = probe_mock(&mut transport, &options(1));

        assert!(text.contains("report descriptor: unavailable ("), "{text}");
        assert!(
            text.contains("feature report 7 (rgb) readback failed: "),
            "{text}"
        );
        assert!(text.contains("verdict: "), "{text}");
    }

    #[test]
    fn the_probe_never_writes_to_the_device() {
        let mut transport = mock()
            .with_report_descriptor(&[0x06, 0x00, 0xFF])
            .push_feature_report(&[0x07, 0x6E, 0xFF, 0x5A, 0x00])
            .push_report(&g1())
            .push_input_report(&rest());

        probe_mock(&mut transport, &options(1));

        assert!(transport.writes().is_empty());
        assert!(transport.feature_writes().is_empty());
        // Nor can it start to: this module has no write path at all.
        let src = include_str!("probe.rs");
        let code = &src[..src.find("#[cfg(test)]").expect("test module")];
        assert!(!code.contains("send_feature_report"));
        assert!(!code.contains(".write("));
    }

    #[test]
    fn inventory_lists_sibling_collections_and_marks_the_selected_one() {
        let criteria = g13().match_criteria.device_match();
        let interface = |path: &str, vendor_id, product_id, usage_page| DeviceInfo {
            path: path.to_owned(),
            vendor_id,
            product_id,
            usage_page,
            ..DeviceInfo::default()
        };
        let devices = vec![
            interface("mouse", 0x1234, 0x0001, 0x0001),
            interface("keyboard-page", 0x046D, 0xC21C, 0x0001),
            interface("vendor-page", 0x046D, 0xC21C, 0xFF00),
        ];
        let mut out = Vec::new();

        print_inventory(&devices, &criteria, &mut out).unwrap();

        let text = String::from_utf8(out).unwrap();
        assert!(
            text.contains("interfaces with vid:pid 046d:c21c: 2"),
            "{text}"
        );
        assert!(
            text.contains("[other collection] 046d:c21c  usage_page=0001"),
            "{text}"
        );
        assert!(
            text.contains("[selected] 046d:c21c  usage_page=ff00"),
            "{text}"
        );
        assert!(!text.contains("path=mouse"), "{text}");
    }

    #[test]
    fn inventory_says_when_no_interface_matches() {
        let criteria = g13().match_criteria.device_match();
        let mut out = Vec::new();

        print_inventory(&[], &criteria, &mut out).unwrap();

        let text = String::from_utf8(out).unwrap();
        assert!(
            text.contains("interfaces with vid:pid 046d:c21c: 0"),
            "{text}"
        );
        assert!(
            text.contains("no interface matches 046d:c21c usage_page=ff00"),
            "{text}"
        );
    }
}
