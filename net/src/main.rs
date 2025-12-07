use anyhow::Result;

mod cli;
mod config;
mod enternet;
mod icmp;
mod ip;
mod udp;

use cli::cli::{Mode, parse_cli};
use enternet::datalink::{datalink_recv, datalink_send};
use enternet::net::{iface_ipv4, iface_mac};

fn main() -> Result<()> {
    let args = parse_cli()?;
    let src_mac = iface_mac(&args.iface)?;
    let src_ip = iface_ipv4(&args.iface)?;
    println!(
        "启动参数: iface={} 本机IP={} 本机MAC={}",
        &args.iface,
        crate::enternet::frame::fmt_ipv4(&src_ip),
        crate::enternet::frame::fmt_mac(&src_mac)
    );
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
