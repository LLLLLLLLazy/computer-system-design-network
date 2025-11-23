use anyhow::{Context, Result, anyhow};
use pcap::{Active, Capture, Error as PcapError, Linktype};
use std::{fs::OpenOptions, io::Write, sync::Arc, thread};

use super::open_device;
use crate::{
    enternet::{
        frame::{
            BROADCAST_MAC, CRC_LEN, ETHER_TYPE_IPV4, HEADER_LEN, IPV4_BROADCAST, MAX_FRAME_SIZE,
            MIN_FRAME_SIZE, OUTPUT_FILE, crc32, fmt_ipv4, fmt_mac,
        },
        recv_queue::{QueueError, RecvQueue},
    },
    ip::{Ipv4Reassembler, ReassembledPacket, parse_ipv4_packet},
};

const RECV_QUEUE_CAPACITY: usize = 1024;

pub fn datalink_recv(iface: &str, local_mac: [u8; 6], local_ip: [u8; 4]) -> Result<()> {
    let mut cap = open_device(iface)?;
    if cap.get_datalink() != Linktype::ETHERNET {
        return Err(anyhow!("仅支持 Ethernet 网卡"));
    }
    if let Err(err) = cap.filter("ether proto 0x0800", true) {
        eprintln!("安装 BPF 过滤器失败，将继续捕获所有帧: {err}");
    }

    let queue = Arc::new(RecvQueue::new(RECV_QUEUE_CAPACITY));
    let worker_queue = Arc::clone(&queue);
    let worker = thread::spawn(move || delivery_worker(worker_queue, local_mac, local_ip));

    println!("正在监听 {iface} ... Ctrl+C 结束");
    let recv_result = recv_loop(&mut cap, local_mac, Arc::clone(&queue));

    queue.close();
    worker.join().map_err(|_| anyhow!("交付线程异常退出"))??;

    recv_result
}

fn recv_loop(cap: &mut Capture<Active>, local_mac: [u8; 6], queue: Arc<RecvQueue>) -> Result<()> {
    loop {
        match cap.next_packet() {
            Ok(packet) => {
                if let Some(frame) = handle_frame(packet.data, &local_mac)? {
                    if let Err(err) = queue.push(frame) {
                        match err {
                            QueueError::Full => eprintln!("接收队列已满，丢弃一帧"),
                            QueueError::Closed => return Ok(()),
                        }
                    }
                }
            }
            Err(PcapError::TimeoutExpired) => continue,
            Err(err) => return Err(err.into()),
        }
    }
}

fn handle_frame(data: &[u8], local_mac: &[u8; 6]) -> Result<Option<Vec<u8>>> {
    if data.len() < HEADER_LEN + CRC_LEN {
        return Ok(None);
    }
    if data.len() < MIN_FRAME_SIZE || data.len() > MAX_FRAME_SIZE {
        //println!("丢弃帧: 长度异常 caplen={}", data.len());
        return Ok(None);
    }
    let dest = &data[..6];
    if dest != local_mac && dest != &BROADCAST_MAC {
        return Ok(None);
    }
    let src = &data[6..12];
    let ether_type = u16::from_be_bytes([data[12], data[13]]);
    let payload_end = data.len() - CRC_LEN;
    let crc_expect = u32::from_be_bytes(data[payload_end..].try_into().expect("slice len checked"));
    let crc_calc = crc32(&data[..payload_end]);
    if crc_calc != crc_expect {
        //println!("丢弃帧: CRC 不匹配 计算={crc_calc:08X} 期望={crc_expect:08X}");
        return Ok(None);
    }
    println!(
        "收到帧: len={} 源MAC={} 目的MAC={} EtherType=0x{ether_type:04X}",
        data.len(),
        fmt_mac(src),
        fmt_mac(dest)
    );
    let frame = data[..payload_end].to_vec();
    Ok(Some(frame))
}

fn delivery_worker(queue: Arc<RecvQueue>, local_mac: [u8; 6], local_ip: [u8; 4]) -> Result<()> {
    let mut reassembler = Ipv4Reassembler::new();
    while let Some(frame) = queue.pop() {
        if let Err(err) = process_frame(&frame, local_mac, local_ip, &mut reassembler) {
            eprintln!("处理帧失败: {err}");
        }
    }
    Ok(())
}

fn process_frame(
    frame: &[u8],
    _local_mac: [u8; 6],
    local_ip: [u8; 4],
    reassembler: &mut Ipv4Reassembler,
) -> Result<()> {
    if frame.len() < HEADER_LEN {
        return Ok(());
    }
    let ether_type = u16::from_be_bytes([frame[12], frame[13]]);
    if ether_type != ETHER_TYPE_IPV4 {
        return Ok(());
    }
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
        "IPv4 首部{fragment_label}: 版本={} IHL={}({}B) ToS=0x{:02X} 标识={} DF={} MF={} 片偏移={}B TTL={} 协议={} 源={} 目的={} 总长={} 选项={}B 载荷={}B",
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
        fmt_ipv4(&header.src),
        fmt_ipv4(&header.dst),
        header.total_length,
        parsed.options.len(),
        parsed.payload.len()
    );

    if let Some(packet) = reassembler.push_fragment(header, parsed.payload) {
        deliver_ip_payload(&packet)?;
    }
    Ok(())
}

fn deliver_ip_payload(packet: &ReassembledPacket) -> Result<()> {
    let protocol = packet.header.protocol;
    let name = protocol_name(protocol);
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
