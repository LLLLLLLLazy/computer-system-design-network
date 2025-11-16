use anyhow::Result;

mod cli;
mod datalink;
mod frame;

use cli::{parse_cli, Mode};
use datalink::{datalink_recv, datalink_send};

fn main() -> Result<()> {
    let args = parse_cli()?;
    match args.mode {
        Mode::Send => datalink_send(&args.iface),
        Mode::Recv => datalink_recv(&args.iface),
    }
}