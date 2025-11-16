//// filepath: /Users/lazy/code/network/enternet/src/net.rs
use anyhow::{anyhow, Result};
use nix::{ifaddrs::getifaddrs, sys::socket::SockaddrLike};

pub fn iface_mac(iface: &str) -> Result<[u8; 6]> {
    for ifaddr in getifaddrs()? {
        if ifaddr.interface_name == iface {
            if let Some(addr) = ifaddr.address {
                if let Some(link) = addr.as_link_addr() {
                    if let Some(mac) = link.addr() {
                        let bytes = mac
                            .get(..6)
                            .ok_or_else(|| anyhow!("无法读取 {iface} 的 MAC"))?;
                        return Ok(bytes.try_into().unwrap());
                    }
                }
            }
        }
    }
    Err(anyhow!("未找到接口 {iface} 的硬件地址"))
}

pub fn parse_mac(input: &str) -> Result<[u8; 6]> {
    let bytes: Result<Vec<u8>, _> = input
        .split(':')
        .map(|part| u8::from_str_radix(part, 16))
        .collect();
    let bytes = bytes.map_err(|_| anyhow!("MAC 地址格式应为 xx:xx:xx:xx:xx:xx"))?;
    if bytes.len() != 6 {
        return Err(anyhow!("MAC 地址必须是 6 个字节"));
    }
    Ok(bytes.try_into().unwrap())
}