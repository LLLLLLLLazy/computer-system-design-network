use anyhow::Result;

mod cli;
mod datalink;
mod frame;
mod net;

use cli::{parse_cli, Mode};
use datalink::{datalink_recv, datalink_send};
use net::iface_mac;

fn main() -> Result<()> {
    let args = parse_cli()?;
    let src_mac = iface_mac(&args.iface)?;
    match args.mode {
        Mode::Send { dest_mac } => datalink_send(&args.iface, src_mac, dest_mac),
        Mode::Recv => datalink_recv(&args.iface, src_mac),
    }
}