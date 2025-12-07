use anyhow::{Context, Result};

use crate::{
    enternet::{
        datalink::open_device,
        frame::{BROADCAST_MAC, ETHER_TYPE_IPV4, build_frame, fmt_ipv4, fmt_mac},
    },
    ip::{Ipv4BuildParams, Ipv4Header, build_ipv4_packets},
};

const ICMP_TYPE_ECHO_REPLY: u8 = 0;
const ICMP_TYPE_ECHO_REQUEST: u8 = 8;
const ICMP_HEADER_LEN: usize = 8;
const DEFAULT_TTL: u8 = 64;

pub fn handle_icmpv4(
    iface: &str,
    local_mac: [u8; 6],
    local_ip: [u8; 4],
    peer_mac: [u8; 6],
    ipv4_header: &Ipv4Header,
    payload: &[u8],
) -> Result<()> {
    if payload.len() < ICMP_HEADER_LEN {
        return Ok(());
    }

    if peer_mac == BROADCAST_MAC {
        return Ok(());
    }

    let icmp_type = payload[0];
    let icmp_code = payload[1];

    if icmp_type != ICMP_TYPE_ECHO_REQUEST || icmp_code != 0 {
        return Ok(());
    }

    if ipv4_header.dst != local_ip {
        return Ok(());
    }

    if !icmp_checksum_valid(payload) {
        eprintln!(
            "ICMP: 校验和错误，丢弃来自 {} 的报文",
            fmt_ipv4(&ipv4_header.src)
        );
        return Ok(());
    }

    let identifier = u16::from_be_bytes([payload[4], payload[5]]);
    let sequence = u16::from_be_bytes([payload[6], payload[7]]);
    println!(
        "[ICMP] 收到 Echo 请求 ▶ 源IP={} ID={} 序号={} 数据={}B",
        fmt_ipv4(&ipv4_header.src),
        identifier,
        sequence,
        payload.len().saturating_sub(ICMP_HEADER_LEN)
    );

    let mut reply = payload.to_vec();
    reply[0] = ICMP_TYPE_ECHO_REPLY;
    reply[1] = 0;
    reply[2] = 0;
    reply[3] = 0;
    let checksum = icmp_checksum(&reply);
    reply[2..4].copy_from_slice(&checksum.to_be_bytes());

    let params = Ipv4BuildParams {
        src: local_ip,
        dst: ipv4_header.src,
        protocol: ipv4_header.protocol,
        ttl: DEFAULT_TTL,
        tos: ipv4_header.tos,
        df: false,
        identification: None,
    };

    let packets = build_ipv4_packets(&reply, &params)?;
    let mut handle = open_device(iface)?;

    for fragment in packets {
        let frame = build_frame(&peer_mac, &local_mac, ETHER_TYPE_IPV4, &fragment.bytes);
        handle
            .sendpacket(frame.as_slice())
            .with_context(|| format!("发送 ICMP Echo 应答失败 (iface={iface})"))?;
        println!(
            "[ICMP] 已发送 Echo 应答 ▶ 目标IP={} 帧长={}B 目的MAC={}",
            fmt_ipv4(&ipv4_header.src),
            frame.len(),
            fmt_mac(&peer_mac)
        );
    }

    Ok(())
}

fn icmp_checksum_valid(payload: &[u8]) -> bool {
    if payload.len() < ICMP_HEADER_LEN {
        return false;
    }
    let mut buf = payload.to_vec();
    buf[2] = 0;
    buf[3] = 0;
    icmp_checksum(&buf) == u16::from_be_bytes([payload[2], payload[3]])
}

fn icmp_checksum(data: &[u8]) -> u16 {
    let mut sum = 0u32;
    let mut chunks = data.chunks_exact(2);
    for chunk in &mut chunks {
        let word = u16::from_be_bytes([chunk[0], chunk[1]]) as u32;
        sum = sum.wrapping_add(word);
    }
    if let Some(&byte) = chunks.remainder().first() {
        sum = sum.wrapping_add((byte as u32) << 8);
    }
    while (sum >> 16) != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }
    !(sum as u16)
}
