use anyhow::Result;

mod enternet;
mod ip;

use enternet::cli::{Mode, parse_cli};
use enternet::datalink::{datalink_recv, datalink_send};
use enternet::net::{iface_ipv4, iface_mac};

fn main() -> Result<()> {
    let args = parse_cli()?;
    let src_mac = iface_mac(&args.iface)?;
    let src_ip = iface_ipv4(&args.iface)?;
    match args.mode {
        Mode::Send {
            dest_mac,
            dest_ip,
            protocol,
        } => datalink_send(&args.iface, src_mac, src_ip, dest_mac, dest_ip, protocol),
        Mode::Recv => datalink_recv(&args.iface, src_mac, src_ip),
    }
}
