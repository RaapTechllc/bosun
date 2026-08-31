//! `bosunctl` — the Bosun control CLI.
//!
//! M1 scope. The CLI names devices by match criteria on the command line; no
//! product identifiers are compiled in.

mod listing;

use anyhow::{Context, Result};
use bosun_hid::{DeviceInfo, HidTransport};
use clap::{Args, Parser, Subcommand};
use tracing_subscriber::EnvFilter;

use crate::listing::{accepts, format_device, parse_u16};

#[derive(Debug, Parser)]
#[command(name = "bosunctl", version, about = "Bosun control CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Inspect HID devices.
    Device {
        #[command(subcommand)]
        command: DeviceCommand,
    },
}

#[derive(Debug, Subcommand)]
enum DeviceCommand {
    /// List enumerated HID interfaces, optionally filtered.
    List(ListArgs),
}

#[derive(Debug, Args)]
struct ListArgs {
    /// Vendor ID, decimal or 0x-prefixed hex.
    #[arg(long, value_parser = parse_u16)]
    vid: Option<u16>,

    /// Product ID, decimal or 0x-prefixed hex.
    #[arg(long, value_parser = parse_u16)]
    pid: Option<u16>,

    /// HID usage page, decimal or 0x-prefixed hex.
    #[arg(long = "usage-page", value_parser = parse_u16)]
    usage_page: Option<u16>,
}

impl ListArgs {
    fn selects(&self, info: &DeviceInfo) -> bool {
        accepts(self.vid, info.vendor_id)
            && accepts(self.pid, info.product_id)
            && accepts(self.usage_page, info.usage_page)
    }
}

fn main() -> Result<()> {
    // Logs go to stderr so stdout stays a clean, pipeable listing.
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Command::Device { command } => match command {
            DeviceCommand::List(args) => device_list(&args),
        },
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

#[cfg(test)]
mod tests {
    use super::*;

    use clap::CommandFactory;

    /// Parse an argv into the `device list` arguments it produces.
    fn list_args<const N: usize>(argv: [&str; N]) -> ListArgs {
        match Cli::parse_from(argv).command {
            Command::Device {
                command: DeviceCommand::List(args),
            } => args,
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
    fn a_usage_page_filter_excludes_the_other_collections_of_one_device() {
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

        assert!(args.selects(&g13_vendor_collection()));
        assert!(!args.selects(&DeviceInfo {
            usage_page: 0x0001,
            ..g13_vendor_collection()
        }));
    }

    #[test]
    fn a_malformed_identifier_is_rejected_rather_than_ignored() {
        assert!(Cli::try_parse_from(["bosunctl", "device", "list", "--vid", "zzz"]).is_err());
        assert!(Cli::try_parse_from(["bosunctl", "device", "list", "--vid", "0x10000"]).is_err());
    }
}
