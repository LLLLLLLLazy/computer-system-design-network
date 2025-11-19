use anyhow::{anyhow, Context, Result};
use std::{fs, sync::Arc, thread};

use super::open_device;
use crate::enternet::{
    frame::{
        crc32, CRC_LEN, ETHER_TYPE_IPV4, HEADER_LEN, INPUT_FILE, MAX_PAYLOAD_LEN, MIN_FRAME_SIZE,
        MIN_PAYLOAD_LEN,
    },
    send_queue::{SendQueue, SendQueueError},
};

const SEND_QUEUE_CAPACITY: usize = 256;

pub fn datalink_send(iface: &str, src_mac: [u8; 6], dest_mac: [u8; 6]) -> Result<()> {
    let payload = fs::read(INPUT_FILE)
        .with_context(|| format!("无法打开输入文件 {INPUT_FILE}"))?;
    if payload.is_empty() {
        return Err(anyhow!("输入文件为空，无法发送"));
    }

    let queue = Arc::new(SendQueue::new(SEND_QUEUE_CAPACITY));
    let worker_queue = Arc::clone(&queue);
    let iface_name = iface.to_string();
    let worker = thread::spawn(move || send_worker(iface_name, worker_queue));

    let mut frame_count = 0usize;
    for chunk in payload.chunks(MAX_PAYLOAD_LEN) {
        let frame = build_frame(chunk, &src_mac, &dest_mac);
        match queue.push(frame) {
            Ok(_) => frame_count += 1,
            Err(SendQueueError::Full) => eprintln!("发送队列已满，丢弃一帧"),
            Err(SendQueueError::Closed) => break,
        }
    }

    queue.close();
    worker
        .join()
        .map_err(|_| anyhow!("发送线程异常退出"))??;

    println!(
        "发送任务完成: 接口={iface} 成功排队帧数={frame_count} 数据来源={INPUT_FILE}"
    );
    Ok(())
}

fn build_frame(chunk: &[u8], src_mac: &[u8; 6], dest_mac: &[u8; 6]) -> Vec<u8> {
    let mut payload = chunk.to_vec();
    if payload.len() < MIN_PAYLOAD_LEN {
        payload.resize(MIN_PAYLOAD_LEN, 0x00);
    }

    let mut frame = Vec::with_capacity(HEADER_LEN + payload.len() + CRC_LEN);
    frame.extend_from_slice(dest_mac);
    frame.extend_from_slice(src_mac);
    frame.extend_from_slice(&ETHER_TYPE_IPV4.to_be_bytes());
    frame.extend_from_slice(&payload);

    let crc = crc32(&frame);
    frame.extend_from_slice(&crc.to_be_bytes());

    if frame.len() < MIN_FRAME_SIZE {
        frame.resize(MIN_FRAME_SIZE, 0);
    }
    frame
}

fn send_worker(iface: String, queue: Arc<SendQueue>) -> Result<()> {
    let mut handle = open_device(&iface)?;
    while let Some(frame) = queue.pop() {
        let crc = u32::from_be_bytes(frame[frame.len() - CRC_LEN..].try_into().unwrap());
        handle
            .sendpacket(frame.as_slice())
            .with_context(|| format!("pcap_sendpacket 失败 (iface={iface})"))?;
        println!("发送帧成功: 接口={iface} 总长={} CRC={crc:08X}", frame.len());
    }
    Ok(())
}