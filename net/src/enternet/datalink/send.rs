use anyhow::{Context, Result, anyhow};
use std::{fs, sync::Arc, thread};

use super::open_device;
use crate::{
    config::network_profile,
    enternet::{
        arp::{self, ArpEntryState, arp_cache, same_subnet},
        frame::{CRC_LEN, ETHER_TYPE_IPV4, INPUT_FILE, build_frame, fmt_ipv4, fmt_mac},
        send_queue::{SendQueue, SendQueueError},
    },
    ip::{Ipv4BuildParams, Ipv4Packet, build_ipv4_packets},
};

const SEND_QUEUE_CAPACITY: usize = 256;
const DEFAULT_TTL: u8 = 64;
const DEFAULT_TOS: u8 = 0;

pub fn datalink_send(
    iface: &str,
    src_mac: [u8; 6],
    src_ip: [u8; 4],
    dest_ip: [u8; 4],
    protocol: u8,
    manual_dest_mac: Option<[u8; 6]>,
) -> Result<()> {
    let payload = fs::read(INPUT_FILE).with_context(|| format!("无法打开输入文件 {INPUT_FILE}"))?;
    if payload.is_empty() {
        return Err(anyhow!("输入文件为空，无法发送"));
    }

    let net_profile = network_profile();
    println!(
        "网络参数: 本地IP={} 子网掩码={} 网关={} DHCP={} DNS=[{}, {}]",
        fmt_ipv4(&src_ip),
        fmt_ipv4(&net_profile.subnet_mask),
        fmt_ipv4(&net_profile.gateway_ip),
        if net_profile.dhcp_enabled {
            "启用"
        } else {
            "关闭"
        },
        fmt_ipv4(&net_profile.dns_servers[0]),
        fmt_ipv4(&net_profile.dns_servers[1])
    );

    let target_ip = if same_subnet(&src_ip, &dest_ip, &net_profile.subnet_mask) {
        dest_ip
    } else {
        println!(
            "目的 IP {} 不在本地网段，使用下一跳 {}",
            fmt_ipv4(&dest_ip),
            fmt_ipv4(&net_profile.gateway_ip)
        );
        net_profile.gateway_ip
    };

    let dest_mac = if let Some(mac) = manual_dest_mac {
        println!("使用手动指定的目的 MAC: {}", fmt_mac(&mac));
        arp_cache().insert(target_ip, mac, ArpEntryState::Static);
        mac
    } else {
        arp::resolve_mac(iface, src_mac, src_ip, target_ip)?
    };

    let params = Ipv4BuildParams {
        src: src_ip,
        dst: dest_ip,
        protocol,
        ttl: DEFAULT_TTL,
        tos: DEFAULT_TOS,
        df: false,
        identification: None,
    };
    let fragments = build_ipv4_packets(&payload, &params)?;
    println!(
        "即将发送 IPv4 数据报: 源IP={} 目的IP={} 协议={} 分片数={}",
        crate::enternet::frame::fmt_ipv4(&src_ip),
        crate::enternet::frame::fmt_ipv4(&dest_ip),
        protocol,
        fragments.len()
    );

    let queue = Arc::new(SendQueue::new(SEND_QUEUE_CAPACITY));
    let worker_queue = Arc::clone(&queue);
    let iface_name = iface.to_string();
    let worker = thread::spawn(move || send_worker(iface_name, worker_queue));

    let mut queued = 0usize;
    for fragment in fragments {
        queued += enqueue_fragment(fragment, &queue, &src_mac, &dest_mac)?;
    }

    queue.close();
    worker.join().map_err(|_| anyhow!("发送线程异常退出"))??;

    println!("发送完成: 成功排队以太网帧 {queued} 个，数据来源={INPUT_FILE}");
    Ok(())
}

fn enqueue_fragment(
    fragment: Ipv4Packet,
    queue: &SendQueue,
    src_mac: &[u8; 6],
    dest_mac: &[u8; 6],
) -> Result<usize> {
    let Ipv4Packet { header, bytes } = fragment;
    let frame = build_frame(dest_mac, src_mac, ETHER_TYPE_IPV4, &bytes);
    println!(
        "构造 IPv4 分片: 标识=0x{:04X}({}) 片偏移={}B ({}×8B) DF={} MF={} 总长={} 载荷={}B",
        header.identification,
        header.identification,
        header.fragment_offset_bytes(),
        header.fragment_offset,
        header.df as u8,
        header.mf as u8,
        header.total_length,
        (header.total_length as usize).saturating_sub(header.header_len_bytes()),
    );
    match queue.push(frame) {
        Ok(_) => Ok(1),
        Err(SendQueueError::Full) => {
            eprintln!("发送队列已满，丢弃 ID={} 的分片", header.identification);
            Ok(0)
        }
        Err(SendQueueError::Closed) => Ok(0),
    }
}

fn send_worker(iface: String, queue: Arc<SendQueue>) -> Result<()> {
    let mut handle = open_device(&iface)?;
    while let Some(frame) = queue.pop() {
        let crc = u32::from_be_bytes(frame[frame.len() - CRC_LEN..].try_into().unwrap());
        handle
            .sendpacket(frame.as_slice())
            .with_context(|| format!("pcap_sendpacket 失败 (iface={iface})"))?;
        println!(
            "发送帧成功: 接口={iface} 总长={} CRC={crc:08X}",
            frame.len()
        );
    }
    Ok(())
}
