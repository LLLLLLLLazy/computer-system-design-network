use std::{
    collections::HashMap,
    convert::TryInto,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use anyhow::{Context, Result, anyhow};
use pcap::Error as PcapError;

use super::{
    datalink::open_device,
    frame::{BROADCAST_MAC, CRC_LEN, ETHER_TYPE_ARP, HEADER_LEN, build_frame, fmt_ipv4, fmt_mac},
};

const HARDWARE_TYPE_ETHERNET: u16 = 0x0001;
const PROTOCOL_TYPE_IPV4: u16 = 0x0800;
const HARDWARE_ADDR_LEN: u8 = 6;
const PROTOCOL_ADDR_LEN: u8 = 4;
const OPERATION_REQUEST: u16 = 1;
const OPERATION_REPLY: u16 = 2;
const ARP_PACKET_LEN: usize = 28;
const CACHE_TTL: Duration = Duration::from_secs(600);
const ARP_MAX_RETRIES: usize = 3;
const ARP_REPLY_TIMEOUT: Duration = Duration::from_secs(10);

static CACHE: OnceLock<ArpCache> = OnceLock::new();

/// ARP 缓存项状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArpEntryState {
    /// 静态配置，永久有效
    Static = 1,
    /// 动态学习，跟随 TTL 自动失效
    Dynamic = 2,
    #[allow(dead_code)]
    /// 仅记录日志（尚未分配 MAC）
    Log = 3,
}

#[derive(Debug, Clone, Copy)]
struct CacheEntry {
    mac: [u8; 6],
    state: ArpEntryState,
    updated_at: Instant,
}

/// 全局 ARP 缓存
pub struct ArpCache {
    entries: Mutex<HashMap<[u8; 4], CacheEntry>>,
}

impl Default for ArpCache {
    fn default() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
        }
    }
}

impl ArpCache {
    pub fn lookup(&self, ip: &[u8; 4]) -> Option<[u8; 6]> {
        let mut guard = self.entries.lock().unwrap();
        guard.retain(|_, entry| match entry.state {
            ArpEntryState::Dynamic => entry.updated_at.elapsed() <= CACHE_TTL,
            _ => true,
        });
        guard.get(ip).and_then(|entry| match entry.state {
            ArpEntryState::Log => None,
            _ => Some(entry.mac),
        })
    }

    pub fn insert(&self, ip: [u8; 4], mac: [u8; 6], state: ArpEntryState) {
        let mut guard = self.entries.lock().unwrap();
        println!(
            "ARP 缓存更新: IP {} -> MAC {} 状态={:?}",
            fmt_ipv4(&ip),
            fmt_mac(&mac),
            state
        );
        guard.insert(
            ip,
            CacheEntry {
                mac,
                state,
                updated_at: Instant::now(),
            },
        );
    }
}

/// 获取全局 ARP 缓存实例
pub fn arp_cache() -> &'static ArpCache {
    CACHE.get_or_init(ArpCache::default)
}

/// 计算网络地址（IP 与子网掩码按位与）
pub fn network_address(ip: &[u8; 4], mask: &[u8; 4]) -> [u8; 4] {
    [
        ip[0] & mask[0],
        ip[1] & mask[1],
        ip[2] & mask[2],
        ip[3] & mask[3],
    ]
}

/// 判断两个 IP 是否在同一网段
pub fn same_subnet(a: &[u8; 4], b: &[u8; 4], mask: &[u8; 4]) -> bool {
    network_address(a, mask) == network_address(b, mask)
}

/// 处理接收到的 ARP 报文，必要时返回需要发送的应答帧
pub fn handle_incoming(
    payload: &[u8],
    local_mac: [u8; 6],
    local_ip: [u8; 4],
) -> Result<Option<Vec<u8>>> {
    if payload.len() < ARP_PACKET_LEN {
        return Ok(None);
    }
    let packet =
        parse_arp_payload(&payload[..ARP_PACKET_LEN]).ok_or_else(|| anyhow!("ARP 报文长度不足"))?;

    arp_cache().insert(packet.sender_ip, packet.sender_mac, ArpEntryState::Dynamic);

    match packet.operation {
        OPERATION_REQUEST if packet.target_ip == local_ip => {
            println!(
                "收到 ARP 请求: {} 请求 {} 的 MAC",
                fmt_ipv4(&packet.sender_ip),
                fmt_ipv4(&packet.target_ip)
            );
            let frame = build_reply_frame(local_mac, local_ip, packet.sender_mac, packet.sender_ip);
            Ok(Some(frame))
        }
        OPERATION_REPLY => {
            println!(
                "收到 ARP 应答: {} -> {}",
                fmt_ipv4(&packet.sender_ip),
                fmt_mac(&packet.sender_mac)
            );
            Ok(None)
        }
        _ => Ok(None),
    }
}

/// 通过 ARP 请求解析目标 IP 对应的 MAC 地址（带缓存与重试）
pub fn resolve_mac(
    iface: &str,
    src_mac: [u8; 6],
    src_ip: [u8; 4],
    target_ip: [u8; 4],
) -> Result<[u8; 6]> {
    if let Some(mac) = arp_cache().lookup(&target_ip) {
        println!(
            "ARP 缓存命中: IP {} -> MAC {}",
            fmt_ipv4(&target_ip),
            fmt_mac(&mac)
        );
        return Ok(mac);
    }

    let mut cap = open_device(iface)?;
    if let Err(err) = cap.filter("arp", true) {
        eprintln!("ARP 解析时安装 BPF 过滤器失败，将监听所有帧: {err}");
    }

    for attempt in 1..=ARP_MAX_RETRIES {
        let frame = build_request_frame(src_mac, src_ip, target_ip);
        cap.sendpacket(frame.as_slice())
            .with_context(|| format!("pcap_sendpacket 发送 ARP 请求失败 (iface={iface})"))?;

        println!(
            "ARP 请求({attempt}/{ARP_MAX_RETRIES}): 解析 IP {}",
            fmt_ipv4(&target_ip)
        );

        let deadline = Instant::now() + ARP_REPLY_TIMEOUT;
        loop {
            if Instant::now() >= deadline {
                break;
            }
            match cap.next_packet() {
                Ok(packet) => {
                    if let Some(parsed) = parse_arp_frame(packet.data) {
                        arp_cache().insert(
                            parsed.sender_ip,
                            parsed.sender_mac,
                            ArpEntryState::Dynamic,
                        );
                        if parsed.operation == OPERATION_REPLY && parsed.sender_ip == target_ip {
                            println!(
                                "ARP 解析成功: IP {} -> MAC {}",
                                fmt_ipv4(&target_ip),
                                fmt_mac(&parsed.sender_mac)
                            );
                            return Ok(parsed.sender_mac);
                        }
                    }
                }
                Err(PcapError::TimeoutExpired) => continue,
                Err(other) => return Err(anyhow!("捕获 ARP 应答失败: {other}")),
            }
        }

        println!(
            "ARP 请求({attempt}) 超时，未在 {:?} 内收到应答，将重试",
            ARP_REPLY_TIMEOUT
        );
    }

    Err(anyhow!(
        "ARP 请求失败: 超过最大重试次数，无法解析 {}",
        fmt_ipv4(&target_ip)
    ))
}

fn build_request_frame(src_mac: [u8; 6], src_ip: [u8; 4], target_ip: [u8; 4]) -> Vec<u8> {
    let payload = build_arp_payload(OPERATION_REQUEST, src_mac, src_ip, [0; 6], target_ip);
    build_frame(&BROADCAST_MAC, &src_mac, ETHER_TYPE_ARP, &payload)
}

fn build_reply_frame(
    local_mac: [u8; 6],
    local_ip: [u8; 4],
    target_mac: [u8; 6],
    target_ip: [u8; 4],
) -> Vec<u8> {
    let payload = build_arp_payload(OPERATION_REPLY, local_mac, local_ip, target_mac, target_ip);
    build_frame(&target_mac, &local_mac, ETHER_TYPE_ARP, &payload)
}

fn build_arp_payload(
    operation: u16,
    sender_mac: [u8; 6],
    sender_ip: [u8; 4],
    target_mac: [u8; 6],
    target_ip: [u8; 4],
) -> [u8; ARP_PACKET_LEN] {
    let mut buf = [0u8; ARP_PACKET_LEN];
    buf[0..2].copy_from_slice(&HARDWARE_TYPE_ETHERNET.to_be_bytes());
    buf[2..4].copy_from_slice(&PROTOCOL_TYPE_IPV4.to_be_bytes());
    buf[4] = HARDWARE_ADDR_LEN;
    buf[5] = PROTOCOL_ADDR_LEN;
    buf[6..8].copy_from_slice(&operation.to_be_bytes());
    buf[8..14].copy_from_slice(&sender_mac);
    buf[14..18].copy_from_slice(&sender_ip);
    buf[18..24].copy_from_slice(&target_mac);
    buf[24..28].copy_from_slice(&target_ip);
    buf
}

fn parse_arp_frame(data: &[u8]) -> Option<ArpPacket> {
    if data.len() < HEADER_LEN + ARP_PACKET_LEN + CRC_LEN {
        return None;
    }
    let ether_type = u16::from_be_bytes([data[12], data[13]]);
    if ether_type != ETHER_TYPE_ARP {
        return None;
    }
    let payload = &data[HEADER_LEN..data.len() - CRC_LEN];
    parse_arp_payload(&payload[..ARP_PACKET_LEN])
}

fn parse_arp_payload(payload: &[u8]) -> Option<ArpPacket> {
    if payload.len() < ARP_PACKET_LEN {
        return None;
    }
    let hardware_type = u16::from_be_bytes([payload[0], payload[1]]);
    let protocol_type = u16::from_be_bytes([payload[2], payload[3]]);
    let operation = u16::from_be_bytes([payload[6], payload[7]]);

    Some(ArpPacket {
        _hardware_type: hardware_type,
        _protocol_type: protocol_type,
        _hardware_len: payload[4],
        _protocol_len: payload[5],
        operation,
        sender_mac: payload[8..14].try_into().unwrap(),
        sender_ip: payload[14..18].try_into().unwrap(),
        _target_mac: payload[18..24].try_into().unwrap(),
        target_ip: payload[24..28].try_into().unwrap(),
    })
}

#[derive(Debug, Clone, Copy)]
struct ArpPacket {
    _hardware_type: u16,
    _protocol_type: u16,
    _hardware_len: u8,
    _protocol_len: u8,
    operation: u16,
    sender_mac: [u8; 6],
    sender_ip: [u8; 4],
    _target_mac: [u8; 6],
    target_ip: [u8; 4],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subnet_detection() {
        let mask = [255, 255, 255, 0];
        let a = [192, 168, 1, 10];
        let b = [192, 168, 1, 200];
        let c = [192, 168, 2, 1];
        assert!(same_subnet(&a, &b, &mask));
        assert!(!same_subnet(&a, &c, &mask));
    }
}
