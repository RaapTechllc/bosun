//! Hardware-in-the-loop tests.
//!
//! Ignored by default and additionally gated on `BOSUN_HW=1`, per
//! `CONTRIBUTING.md`. With the device attached:
//!
//! ```text
//! BOSUN_HW=1 cargo test -p bosun-hid --test hardware -- --ignored --nocapture
//! ```
//!
//! [`r1_shared_input_reports_reach_a_second_reader`] is risk R1 from
//! `docs/BOSUN-PLAN.md`: it answers whether a second reader receives input
//! reports while Logitech Gaming Software holds its own handle. Run it once
//! with LGS running and, if no reports arrive, again with LGS closed.

use std::time::{Duration, Instant};

use bosun_hid::{HidTransport, ReadOutcome, Transport};

/// Both gates must be open: `#[ignore]` keeps this out of normal runs, and
/// `BOSUN_HW=1` keeps `--ignored` runs on machines without hardware honest.
fn hardware_enabled() -> bool {
    match std::env::var("BOSUN_HW") {
        Ok(value) if value == "1" => true,
        _ => {
            eprintln!("skipped: set BOSUN_HW=1 with the device attached");
            false
        }
    }
}

#[test]
#[ignore = "requires attached hardware; run with BOSUN_HW=1"]
fn the_backend_enumerates_hid_interfaces() {
    if !hardware_enabled() {
        return;
    }

    let api = HidTransport::api().expect("the HID backend initialises");
    let devices = HidTransport::enumerate(&api);

    for info in &devices {
        eprintln!(
            "{:04x}:{:04x} usage_page={:04x} path={}",
            info.vendor_id, info.product_id, info.usage_page, info.path
        );
    }

    assert!(
        !devices.is_empty(),
        "no HID interfaces enumerated; on Linux check the udev rule"
    );
}

#[test]
#[ignore = "requires attached hardware; run with BOSUN_HW=1 and BOSUN_HW_PATH"]
fn r1_shared_input_reports_reach_a_second_reader() {
    if !hardware_enabled() {
        return;
    }

    let Ok(path) = std::env::var("BOSUN_HW_PATH") else {
        eprintln!("skipped: set BOSUN_HW_PATH to a path from `bosunctl device list`");
        return;
    };

    let api = HidTransport::api().expect("the HID backend initialises");
    let mut transport = HidTransport::open_path(&api, &path).expect("the interface opens");

    eprintln!("press keys on the device for the next 15 seconds");

    let deadline = Instant::now() + Duration::from_secs(15);
    let mut buf = [0u8; 64];
    let mut reports = 0usize;

    while Instant::now() < deadline {
        match transport.read(&mut buf, Duration::from_millis(500)) {
            Ok(ReadOutcome::Report(len)) => {
                reports += 1;
                eprintln!("report {reports}: {:02x?}", &buf[..len]);
            }
            Ok(ReadOutcome::Timeout) => {}
            Err(error) => panic!("read failed after {reports} reports: {error}"),
        }
    }

    assert!(
        reports > 0,
        "no input reports arrived in 15 s. If LGS is running, close it and \
         re-run before concluding that shared reads do not work (risk R1)."
    );
}
