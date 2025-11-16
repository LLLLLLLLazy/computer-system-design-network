use anyhow::{anyhow, Context, Result};
use std::fs;

use super::open_device;
use crate::frame::{
    crc32, CRC_LEN, ETHER_TYPE_IPV4, HEADER_LEN, INPUT_FILE, MAX_PAYLOAD_LEN,
    MIN_FRAME_SIZE, MIN_PAYLOAD_LEN,
};

pub fn datalink_send(iface: &str, src_mac: [u8; 6], dest_mac: [u8; 6]) -> Result<()> {
    let mut payload = fs::read(INPUT_FILE)
        .with_context(|| format!("无法打开输入文件 {INPUT_FILE}"))?;
    if payload.is_empty() {
        return Err(anyhow!("输入文件为空，无法发送"));
    }
    if payload.len() > MAX_PAYLOAD_LEN {
        return Err(anyhow!("输入数据超过 {MAX_PAYLOAD_LEN} 字节，拒绝发送"));
    }
    if payload.len() < MIN_PAYLOAD_LEN {
        payload.resize(MIN_PAYLOAD_LEN, 0x00);
        println!("已对输入数据填充至 {} 字节", payload.len());
    }

    let mut frame = Vec::with_capacity(HEADER_LEN + payload.len() + CRC_LEN);
    frame.extend_from_slice(&dest_mac);
    frame.extend_from_slice(&src_mac);
    frame.extend_from_slice(&ETHER_TYPE_IPV4.to_be_bytes());
    frame.extend_from_slice(&payload);
    let crc = crc32(&frame);
    frame.extend_from_slice(&crc.to_be_bytes());
    if frame.len() < MIN_FRAME_SIZE {
        frame.resize(MIN_FRAME_SIZE, 0);
    }

    let mut handle = open_device(iface)?;
    handle
        .sendpacket(frame.as_slice())
        .with_context(|| format!("pcap_sendpacket 失败 (iface={iface})"))?;
    println!(
        "发送帧成功: 接口={iface} 载荷={} CRC={crc:08X} 总长={} 数据来源={INPUT_FILE}",
        payload.len(),
        frame.len()
    );
    Ok(())
}