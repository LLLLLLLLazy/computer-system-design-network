use anyhow::{Context, Result, anyhow};
use pcap::{Active, Capture, Error as PcapError};
use std::{convert::TryInto, fs::OpenOptions, io::Write, sync::Arc, thread};

use crate::{
    enternet::{
        arp,
        frame::{
            BROADCAST_MAC, CRC_LEN, ETHER_TYPE_ARP, ETHER_TYPE_IPV4, HEADER_LEN, IPV4_BROADCAST,
            MAX_FRAME_SIZE, MIN_FRAME_SIZE, OUTPUT_FILE, crc32, fmt_ipv4, fmt_mac,
        },
        recv_queue::{QueueError, RecvQueue},
    },
    icmp::handle_icmpv4,
    ip::{Ipv4Reassembler, ReassembledPacket, parse_ipv4_packet},
    udp::handle_udp_packet,
};

const RECV_QUEUE_CAPACITY: usize = 1024;

enum FrameDispatch {
    DeliverIpv4(Vec<u8>),
    Reply(Vec<u8>),
    Ignore,
}

pub fn datalink_recv(iface: &str, local_mac: [u8; 6], local_ip: [u8; 4]) -> Result<()> {
    // 直接使用 iface 字符串打开 pcap，避免类型转换问题
    // 确保使用更宽的 snaplen 与包含 arp 的过滤器
    let snaplen: i32 = 524288;
    let promiscuous = true;
    let timeout_ms = 1000;
    let mut cap = pcap::Capture::from_device(iface)?
        .snaplen(snaplen)
        .promisc(promiscuous)
        .timeout(timeout_ms)
        .open()?;

    // 允许捕获 ARP + IPv4
    let bpf_filter = "arp or ether proto 0x0800";
    cap.filter(bpf_filter, true)?;

    let queue = Arc::new(RecvQueue::new(RECV_QUEUE_CAPACITY));
    let worker_queue = Arc::clone(&queue);
    let iface_name = iface.to_string();
        println!(
            "监听信息: iface={} 本机IP={} 本机MAC={}",
            iface,
            fmt_ipv4(&local_ip),
            fmt_mac(&local_mac)
        );
    let worker =
        thread::spawn(move || delivery_worker(worker_queue, iface_name, local_mac, local_ip));

    println!("正在监听 {iface} ... Ctrl+C 结束");
    let recv_result = recv_loop(iface, &mut cap, local_mac, local_ip, Arc::clone(&queue));

    queue.close();
    worker.join().map_err(|_| anyhow!("交付线程异常退出"))??;

    recv_result
}

fn recv_loop(
    iface: &str,
    cap: &mut Capture<Active>,
    local_mac: [u8; 6],
    local_ip: [u8; 4],
    queue: Arc<RecvQueue>,
) -> Result<()> {
    loop {
        match cap.next_packet() {
            Ok(packet) => match handle_frame(packet.data, &local_mac, &local_ip)? {
                FrameDispatch::DeliverIpv4(frame) => {
                    if let Err(err) = queue.push(frame) {
                        match err {
                            QueueError::Full => eprintln!("接收队列已满，丢弃一帧"),
                            QueueError::Closed => return Ok(()),
                        }
                    }
                }
                FrameDispatch::Reply(reply) => {
                    cap.sendpacket(reply.as_slice())
                        .with_context(|| format!("发送 ARP 应答失败 (iface={iface})"))?;
                    println!(
                        "ARP 应答已发送: 目标MAC={} 帧长={}",
                        fmt_mac(&reply[..6]),
                        reply.len()
                    );
                }
                FrameDispatch::Ignore => {}
            },
            Err(PcapError::TimeoutExpired) => continue,
            Err(err) => return Err(err.into()),
        }
    }
}

fn handle_frame(data: &[u8], local_mac: &[u8; 6], local_ip: &[u8; 4]) -> Result<FrameDispatch> {
    if data.len() < HEADER_LEN + CRC_LEN {
        return Ok(FrameDispatch::Ignore);
    }
    if data.len() < MIN_FRAME_SIZE || data.len() > MAX_FRAME_SIZE {
        //println!("丢弃帧: 长度异常 caplen={}", data.len());
        return Ok(FrameDispatch::Ignore);
    }
    let dest = &data[..6];
    if dest != local_mac && dest != &BROADCAST_MAC {
        return Ok(FrameDispatch::Ignore);
    }
    let src = &data[6..12];
    let ether_type = u16::from_be_bytes([data[12], data[13]]);
    let payload_end = data.len() - CRC_LEN;
    let crc_expect = u32::from_be_bytes(data[payload_end..].try_into().expect("slice len checked"));
    let crc_calc = crc32(&data[..payload_end]);
    if crc_calc != crc_expect {
        //println!("丢弃帧: CRC 不匹配 计算={crc_calc:08X} 期望={crc_expect:08X}");
        return Ok(FrameDispatch::Ignore);
    }
    println!(
        "收到帧: len={} 源MAC={} 目的MAC={} EtherType=0x{ether_type:04X}",
        data.len(),
        fmt_mac(src),
        fmt_mac(dest)
    );
    let frame = data[..payload_end].to_vec();
    match ether_type {
        ETHER_TYPE_IPV4 => Ok(FrameDispatch::DeliverIpv4(frame)),
        ETHER_TYPE_ARP => {
            let payload = &frame[HEADER_LEN..];
            match arp::handle_incoming(payload, *local_mac, *local_ip) {
                Ok(Some(reply)) => Ok(FrameDispatch::Reply(reply)),
                Ok(None) => Ok(FrameDispatch::Ignore),
                Err(err) => {
                    eprintln!("ARP 处理失败: {err}");
                    Ok(FrameDispatch::Ignore)
                }
            }
        }
        _ => Ok(FrameDispatch::Ignore),
    }
}

fn delivery_worker(
    queue: Arc<RecvQueue>,
    iface: String,
    local_mac: [u8; 6],
    local_ip: [u8; 4],
) -> Result<()> {
    let mut reassembler = Ipv4Reassembler::new();
    while let Some(frame) = queue.pop() {
        if let Err(err) = process_frame(&frame, local_mac, local_ip, &iface, &mut reassembler) {
            eprintln!("处理帧失败: {err}");
        }
    }
    Ok(())
}

fn process_frame(
    frame: &[u8],
    local_mac: [u8; 6],
    local_ip: [u8; 4],
    iface: &str,
    reassembler: &mut Ipv4Reassembler,
) -> Result<()> {
    if frame.len() < HEADER_LEN {
        return Ok(());
    }
    let ether_type = u16::from_be_bytes([frame[12], frame[13]]);
    if ether_type != ETHER_TYPE_IPV4 {
        return Ok(());
    }
    let peer_mac: [u8; 6] = frame[6..12].try_into().expect("slice len checked");
    for expired in reassembler.remove_expired() {
        println!(
            "分片重组超时: 标识={} 源={} 目的={} 协议={}",
            expired.identification,
            fmt_ipv4(&expired.src),
            fmt_ipv4(&expired.dst),
            expired.protocol
        );
    }
    let ipv4_bytes = &frame[HEADER_LEN..];
    let parsed = match parse_ipv4_packet(ipv4_bytes) {
        Ok(packet) => packet,
        Err(err) => {
            eprintln!("IPv4 解析失败: {err}");
            return Ok(());
        }
    };
    let header = parsed.header.clone();

    if header.dst != local_ip && header.dst != IPV4_BROADCAST {
        println!(
            "丢弃 IPv4 分组: 目的IP={} 与本机 {} 不匹配",
            fmt_ipv4(&header.dst),
            fmt_ipv4(&local_ip)
        );
        return Ok(());
    }

    let fragment_label = if header.mf || header.fragment_offset > 0 {
        "[分片]"
    } else {
        ""
    };
        println!(
            "IPv4 首部{fragment_label}: 版本={} IHL={}({}B) ToS=0x{:02X} 标识={} DF={} MF={} 片偏移={}B TTL={} 协议={}({}) 源={} 目的={} 总长={} 选项={}B 载荷={}B",
            header.version,
            header.ihl,
            header.header_len_bytes(),
            header.tos,
            header.identification,
            header.df as u8,
            header.mf as u8,
            header.fragment_offset_bytes(),
            header.ttl,
            header.protocol,
            protocol_name(header.protocol),
            fmt_ipv4(&header.src),
            fmt_ipv4(&header.dst),
            header.total_length,
            parsed.options.len(),
            parsed.payload.len()
        );

    if let Some(packet) = reassembler.push_fragment(header, parsed.payload) {
        deliver_ip_payload(&packet, local_mac, local_ip, peer_mac, iface)?;
    }
    Ok(())
}

fn deliver_ip_payload(
    packet: &ReassembledPacket,
    local_mac: [u8; 6],
    local_ip: [u8; 4],
    peer_mac: [u8; 6],
    iface: &str,
) -> Result<()> {
    let protocol = packet.header.protocol;
    let name = protocol_name(protocol);
    match protocol {
        1 => {
            if let Err(err) = handle_icmpv4(
                iface,
                local_mac,
                local_ip,
                peer_mac,
                &packet.header,
                &packet.payload,
            ) {
                eprintln!("ICMP 处理失败: {err}");
            }
        }
        17 => {
            if let Err(err) = handle_udp_packet(&packet.header, &packet.payload) {
                eprintln!("UDP 处理失败: {err}");
            }
            // UDP 走 socket 队列，不直接写输出文件，避免将 UDP 首部落盘
            return Ok(());
        }
        _ => {}
    }
    OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(OUTPUT_FILE)
        .with_context(|| format!("写入 {OUTPUT_FILE} 失败"))?
        .write_all(&packet.payload)
        .with_context(|| format!("覆盖 {OUTPUT_FILE} 失败"))?;
    println!(
        "已交付至上层: 协议={} ({name}) 载荷={} 字节 -> {OUTPUT_FILE}",
        protocol,
        packet.payload.len()
    );
    Ok(())
}

fn protocol_name(protocol: u8) -> &'static str {
    match protocol {
        1 => "ICMPv4",
        2 => "IGMPv4",
        6 => "TCP",
        17 => "UDP",
        _ => "未知",
    }
}
