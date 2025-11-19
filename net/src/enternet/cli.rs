use anyhow::{anyhow, Context, Result};
use pcap::Device;
use std::{
    env,
    io::{self, Write},
};

use crate::enternet::net::parse_mac;

pub enum Mode {
    Send { dest_mac: [u8; 6] },
    Recv,
}

pub struct CliArgs {
    pub mode: Mode,
    pub iface: String,
}

pub fn parse_cli() -> Result<CliArgs> {
    let mut args = env::args();
    let program = args.next().unwrap_or_else(|| "enternet".into());
    let raw_mode = args
        .next()
        .ok_or_else(|| anyhow!("用法: {program} <send|recv> [iface] [dest-mac]"))?;
    let iface = if let Some(i) = args.next() {
        i
    } else {
        choose_interface()?
    };
    let mode = match raw_mode.to_lowercase().as_str() {
        "send" => {
            let dest_arg = args.next().map(|m| parse_mac(&m));
            let dest_mac = match dest_arg {
                Some(Ok(mac)) => mac,
                Some(Err(e)) => return Err(e),
                None => prompt_dest_mac()?,
            };
            Mode::Send { dest_mac }
        }
        "recv" => Mode::Recv,
        _ => return Err(anyhow!("用法: {program} <send|recv> [iface] [dest-mac]")),
    };
    Ok(CliArgs { mode, iface })
}

fn prompt_dest_mac() -> Result<[u8; 6]> {
    print!("请输入目标 MAC (xx:xx:xx:xx:xx:xx): ");
    io::stdout().flush().ok();
    let mut buf = String::new();
    io::stdin().read_line(&mut buf)?;
    parse_mac(buf.trim())
}

fn choose_interface() -> Result<String> {
    let devices = Device::list().context("pcap_findalldevs 失败")?;
    if devices.is_empty() {
        return Err(anyhow!("未发现网卡"));
    }
    for (idx, dev) in devices.iter().enumerate() {
        let desc = dev.desc.as_deref().unwrap_or("无描述");
        println!("{}. {} ({})", idx + 1, dev.name, desc);
    }
    print!("请选择网卡序号 (1-{}): ", devices.len());
    io::stdout().flush().ok();
    let mut buf = String::new();
    io::stdin().read_line(&mut buf)?;
    let choice: usize = buf.trim().parse().context("非法输入")?;
    if choice == 0 || choice > devices.len() {
        return Err(anyhow!("网卡序号超出范围"));
    }
    Ok(devices[choice - 1].name.clone())
}