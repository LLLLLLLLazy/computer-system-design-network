use anyhow::{anyhow, Context, Result};
use pcap::Device;
use std::{
    env,
    io::{self, Write},
};

pub enum Mode {
    Send,
    Recv,
}

pub struct CliArgs {
    pub mode: Mode,
    pub iface: String,
}

pub fn parse_cli() -> Result<CliArgs> {
    let mut args = env::args();
    let program = args.next().unwrap_or_else(|| "enternet".into());
    let mode = match args.next() {
        Some(m) if m.eq_ignore_ascii_case("send") => Mode::Send,
        Some(m) if m.eq_ignore_ascii_case("recv") => Mode::Recv,
        _ => return Err(anyhow!("用法: {program} <send|recv> [iface]")),
    };
    let iface = if let Some(i) = args.next() {
        i
    } else {
        choose_interface()?
    };
    Ok(CliArgs { mode, iface })
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