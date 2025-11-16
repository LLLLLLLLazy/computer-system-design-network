use anyhow::{anyhow, Context, Result};
use pcap::{Active, Capture, Error as PcapError, Linktype};
use std::{
    fs::OpenOptions,
    io::Write,
    sync::Arc,
    thread,
};

use super::open_device;
use crate::{
    frame::{
        crc32, fmt_mac, BROADCAST_MAC, CRC_LEN, HEADER_LEN, MAX_FRAME_SIZE, MIN_FRAME_SIZE,
        OUTPUT_FILE,
    },
    recv_queue::{QueueError, RecvQueue},
};

const RECV_QUEUE_CAPACITY: usize = 1024;

pub fn datalink_recv(iface: &str, local_mac: [u8; 6]) -> Result<()> {
    let mut cap = open_device(iface)?;
    if cap.get_datalink() != Linktype::ETHERNET {
        return Err(anyhow!("仅支持 Ethernet 网卡"));
    }
    if let Err(err) = cap.filter("ether proto 0x0800", true) {
        eprintln!("安装 BPF 过滤器失败，将继续捕获所有帧: {err}");
    }

    let queue = Arc::new(RecvQueue::new(RECV_QUEUE_CAPACITY));
    let worker_queue = Arc::clone(&queue);
    let worker = thread::spawn(move || delivery_worker(worker_queue));

    println!("正在监听 {iface} ... Ctrl+C 结束");
    let recv_result = recv_loop(&mut cap, local_mac, Arc::clone(&queue));

    queue.close();
    worker
        .join()
        .map_err(|_| anyhow!("交付线程异常退出"))??;

    recv_result
}

fn recv_loop(
    cap: &mut Capture<Active>,
    local_mac: [u8; 6],
    queue: Arc<RecvQueue>,
) -> Result<()> {
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
        println!("丢弃帧: 长度异常 caplen={}", data.len());
        return Ok(None);
    }
    let dest = &data[..6];
    if dest != local_mac && dest != &BROADCAST_MAC {
        return Ok(None);
    }
    let src = &data[6..12];
    let ether_type = u16::from_be_bytes([data[12], data[13]]);
    let payload_end = data.len() - CRC_LEN;
    let crc_expect =
        u32::from_be_bytes(data[payload_end..].try_into().expect("slice len checked"));
    let crc_calc = crc32(&data[..payload_end]);
    if crc_calc != crc_expect {
        println!(
            "丢弃帧: CRC 不匹配 计算={crc_calc:08X} 期望={crc_expect:08X}"
        );
        return Ok(None);
    }
    println!(
        "收到帧: len={} 源MAC={} 目的MAC={} 协议类型=0x{ether_type:04X} CRC32={crc_expect:08X} (校验通过)",
        data.len(),
        fmt_mac(src),
        fmt_mac(dest)
    );
    Ok(Some(data.to_vec()))
}

fn delivery_worker(queue: Arc<RecvQueue>) -> Result<()> {
    while let Some(frame) = queue.pop() {
        deliver_frame(&frame)?;
    }
    Ok(())
}

fn deliver_frame(frame: &[u8]) -> Result<()> {
    OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(OUTPUT_FILE)
        .with_context(|| format!("写入 {OUTPUT_FILE} 失败"))?
        .write_all(frame)
        .with_context(|| format!("覆盖 {OUTPUT_FILE} 失败"))?;
    println!("已将完整帧 {} 字节写入 {OUTPUT_FILE}（覆盖模式）", frame.len());
    Ok(())
}