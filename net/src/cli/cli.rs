use anyhow::{Context, Result, anyhow};
use pcap::Device;
use std::{
    env,
    io::{self, Write},
};

use crate::enternet::net::parse_mac;

pub enum Mode {
    Send {
        dest_mac: [u8; 6],
        dest_ip: [u8; 4],
        protocol: u8,
    },
    Recv,
}

pub struct CliArgs {
    pub mode: Mode,
    pub iface: String,
}

pub fn parse_cli() -> Result<CliArgs> {
    let mut args = env::args();
    let program = args.next().unwrap_or_else(|| "enternet".into());
    let raw_mode = args.next().ok_or_else(|| {
        anyhow!("用法: {program} <send|recv> [iface] [dest-mac] [dest-ip] [protocol]")
    })?;
    let mut rest: Vec<String> = args.collect();
    let iface = if let Some(candidate) = rest.first() {
        if looks_like_mac(candidate) || looks_like_ipv4(candidate) {
            choose_interface()?
        } else {
            rest.remove(0)
        }
    } else {
        choose_interface()?
    };
    match raw_mode.to_lowercase().as_str() {
        "send" => {
            if rest.len() > 3 {
                return Err(anyhow!(
                    "参数过多，最多提供网卡、目标 MAC、目标 IP 与协议号"
                ));
            }
            let dest_mac = match rest.get(0) {
                Some(value) => parse_mac(value),
                None => {
                    let input = prompt("请输入目标 MAC (格式 XX:XX:XX:XX:XX:XX): ")?;
                    parse_mac(&input)
                }
            }?;
            let dest_ip = match rest.get(1) {
                Some(value) => parse_ipv4(value)?,
                None => {
                    let input = prompt("请输入目标 IP (格式 a.b.c.d): ")?;
                    parse_ipv4(&input)?
                }
            };
            let protocol = match rest.get(2) {
                Some(value) => parse_protocol(value)?,
                None => {
                    let input =
                        prompt("请输入上层协议号 (ICMP=1, IGMP=2, TCP=6, UDP=17，默认0): ")?;
                    if input.trim().is_empty() {
                        0
                    } else {
                        parse_protocol(&input)?
                    }
                }
            };
            Ok(CliArgs {
                mode: Mode::Send {
                    dest_mac,
                    dest_ip,
                    protocol,
                },
                iface,
            })
        }
        "recv" => Ok(CliArgs {
            mode: Mode::Recv,
            iface,
        }),
        _ => Err(anyhow!(
            "用法: {program} <send|recv> [iface] [dest-mac] [dest-ip] [protocol]"
        )),
    }
}

fn looks_like_mac(input: &str) -> bool {
    input.contains(':')
}

fn looks_like_ipv4(input: &str) -> bool {
    input.split('.').count() == 4
}

fn parse_ipv4(input: &str) -> Result<[u8; 4]> {
    let parts: Vec<&str> = input.split('.').collect();
    if parts.len() != 4 {
        return Err(anyhow!("IPv4 地址格式应为 a.b.c.d"));
    }
    let mut addr = [0u8; 4];
    for (idx, part) in parts.iter().enumerate() {
        let value: u8 = part
            .parse()
            .map_err(|_| anyhow!("IPv4 地址片段 {part} 非法"))?;
        addr[idx] = value;
    }
    Ok(addr)
}

fn parse_protocol(input: &str) -> Result<u8> {
    input.parse::<u8>().context("协议号需为 0-255 的整数")
}

fn prompt(message: &str) -> Result<String> {
    print!("{message}");
    io::stdout().flush().ok();
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    Ok(line.trim().to_string())
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
