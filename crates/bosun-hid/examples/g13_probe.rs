use std::time::{Duration, Instant};

use hidapi::HidApi;

const VID: u16 = 0x046d;
const PID: u16 = 0xc21c;
const USAGE_PAGE: u16 = 0xff00;
const BUFFER_LEN: usize = 64;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api = HidApi::new()?;
    let interfaces = api
        .device_list()
        .filter(|info| info.vendor_id() == VID && info.product_id() == PID)
        .inspect(|info| {
            println!(
                "found path={:?} usage_page=0x{:04x} usage=0x{:04x} interface={}",
                info.path(),
                info.usage_page(),
                info.usage(),
                info.interface_number()
            );
        })
        .collect::<Vec<_>>();

    let info = interfaces
        .into_iter()
        .find(|info| info.usage_page() == USAGE_PAGE)
        .ok_or("G13 vendor HID interface 046d:c21c / ff00 not found")?;
    let device = info.open_device(&api)?;

    println!("opened shared vendor interface; capturing until stopped with Ctrl+C");
    let started = Instant::now();
    let mut last_heartbeat = Instant::now();
    let mut timeout_count = 0_u64;

    loop {
        let mut report = [0_u8; BUFFER_LEN];
        match device.read_timeout(&mut report, 250) {
            Ok(0) => timeout_count += 1,
            Ok(length) => {
                println!(
                    "{:>8}ms n={length:<2} {}",
                    started.elapsed().as_millis(),
                    report[..length]
                        .iter()
                        .map(|byte| format!("{byte:02x}"))
                        .collect::<Vec<_>>()
                        .join(" ")
                );
            }
            Err(error) => return Err(error.into()),
        }

        if last_heartbeat.elapsed() >= Duration::from_secs(5) {
            println!(
                "{:>8}ms waiting ({timeout_count} timeouts so far)",
                started.elapsed().as_millis()
            );
            last_heartbeat = Instant::now();
        }
    }
}
