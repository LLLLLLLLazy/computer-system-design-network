use anyhow::Result;

mod cli;
mod config;
mod enternet;
mod ip;

use cli::cli::{Mode, parse_cli};
use enternet::datalink::{datalink_recv, datalink_send};
use enternet::net::{iface_ipv4, iface_mac};

fn main() -> Result<()> {
    let args = parse_cli()?;
    let src_mac = iface_mac(&args.iface)?;
    let src_ip = iface_ipv4(&args.iface)?;
    match args.mode {
        Mode::Send {
            dest_ip,
            protocol,
            manual_dest_mac,
        } => datalink_send(
            &args.iface,
            src_mac,
            src_ip,
            dest_ip,
            protocol,
            manual_dest_mac,
        ),
        Mode::Recv => datalink_recv(&args.iface, src_mac, src_ip),
    }
}
