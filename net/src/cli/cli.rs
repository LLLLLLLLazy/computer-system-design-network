use anyhow::{Context, Result, anyhow};
use pcap::Device;
use std::{
    env,
    io::{self, Write},
};

use crate::enternet::net::parse_mac;

pub enum Mode {
    Send {
        dest_ip: [u8; 4],
        protocol: u8,
        manual_dest_mac: Option<[u8; 6]>,
    },
    Recv,
    UdpSendFile {
        dest_ip: [u8; 4],
        dest_port: u16,
        src_port: Option<u16>,
        file: String,
    },
    UdpRecvFile {
        listen_port: u16,
        output: String,
    },
}

pub struct CliArgs {
    pub mode: Mode,
    pub iface: String,
}

pub fn parse_cli() -> Result<CliArgs> {
    let mut args = env::args();
    let program = args.next().unwrap_or_else(|| "enternet".into());
    let raw_mode = args.next().ok_or_else(|| {
        anyhow!("用法: {program} <send|recv|udp-send|udp-recv> ...")
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
            if rest.len() > 4 {
                return Err(anyhow!(
                    "参数过多，最多提供网卡、目标 IP、协议号与可选的目标 MAC"
                ));
            }

            let mut manual_dest_mac = None;
            let mut parsed_dest_ip = None;
            let mut parsed_protocol = None;

            for token in &rest {
                if looks_like_mac(token) && manual_dest_mac.is_none() {
                    manual_dest_mac = Some(parse_mac(token)?);
                    continue;
                }
                if looks_like_ipv4(token) && parsed_dest_ip.is_none() {
                    parsed_dest_ip = Some(parse_ipv4(token)?);
                    continue;
                }
                if parsed_protocol.is_none() {
                    parsed_protocol = Some(parse_protocol(token)?);
                    continue;
                }
                return Err(anyhow!(
                    "无法解析参数 {token}，请按照 <iface> [dest-ip] [protocol] [dest-mac] 的顺序"
                ));
            }

            let dest_ip = match parsed_dest_ip {
                Some(ip) => ip,
                None => {
                    let input = prompt("请输入目标 IP (格式 a.b.c.d): ")?;
                    parse_ipv4(&input)?
                }
            };

            let protocol = match parsed_protocol {
                Some(num) => num,
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
                    dest_ip,
                    protocol,
                    manual_dest_mac,
                },
                iface,
            })
        }
        "recv" => Ok(CliArgs {
            mode: Mode::Recv,
            iface,
        }),
        "udp-send" => {
            if rest.len() < 3 || rest.len() > 4 {
                return Err(anyhow!(
                    "用法: {program} udp-send <iface> <dest-ip> <dest-port> <file> [src-port]"
                ));
            }

            // 位置参数: iface dest-ip dest-port file [src-port]
            let dest_ip = parse_ipv4(&rest[0])?;
            let dest_port: u16 = rest[1]
                .parse()
                .context("目标端口需为 0-65535 整数")?;
            let file = rest[2].clone();
            let src_port = if rest.len() == 4 {
                Some(rest[3].parse().context("源端口需为 0-65535 整数")?)
            } else {
                None
            };

            Ok(CliArgs {
                mode: Mode::UdpSendFile {
                    dest_ip,
                    dest_port,
                    src_port,
                    file,
                },
                iface,
            })
        }
        "udp-recv" => {
            if rest.len() < 2 {
                return Err(anyhow!("用法: {program} udp-recv <iface> <listen-port> <output-file>"));
            }
            let listen_port: u16 = rest[0]
                .parse()
                .context("监听端口需为 0-65535 整数")?;
            let output = rest[1].clone();
            Ok(CliArgs {
                mode: Mode::UdpRecvFile { listen_port, output },
                iface,
            })
        }
        _ => Err(anyhow!(
            "用法: {program} <send|recv|udp-send|udp-recv> ..."
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
