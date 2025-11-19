use anyhow::Result;

mod enternet;

use enternet::cli::{parse_cli, Mode};
use enternet::datalink::{datalink_recv, datalink_send};
use enternet::net::iface_mac;

fn main() -> Result<()> {
    let args = parse_cli()?;
    let src_mac = iface_mac(&args.iface)?;
    match args.mode {
        Mode::Send { dest_mac } => datalink_send(&args.iface, src_mac, dest_mac),
        Mode::Recv => datalink_recv(&args.iface, src_mac),
    }
}