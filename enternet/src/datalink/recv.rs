use anyhow::{anyhow, Context, Result};
use pcap::{Error as PcapError, Linktype};
use std::{
    fs::OpenOptions,
    io::Write,
};

use super::open_device;
use crate::frame::{
    crc32, fmt_mac, BROADCAST_MAC, CRC_LEN, DEST_MAC, HEADER_LEN, MAX_FRAME_SIZE, MIN_FRAME_SIZE,
    OUTPUT_FILE,
};

pub fn datalink_recv(iface: &str) -> Result<()> {
    let mut cap = open_device(iface)?;
    if cap.get_datalink() != Linktype::ETHERNET {
        return Err(anyhow!("仅支持 Ethernet 网卡"));
    }
    if let Err(err) = cap.filter("ether proto 0x0800", true) {
        eprintln!("安装 BPF 过滤器失败，将继续捕获所有帧: {err}");
    }
    println!("正在监听 {iface} ... Ctrl+C 结束");
    loop {
        match cap.next_packet() {
            Ok(packet) => {
                if let Err(err) = handle_frame(packet.data) {
                    eprintln!("{err}");
                }
            }
            Err(PcapError::TimeoutExpired) => continue,
            Err(err) => return Err(err.into()),
        }
    }
}

fn handle_frame(data: &[u8]) -> Result<()> {
    if data.len() < HEADER_LEN + CRC_LEN {
        return Ok(());
    }
    if data.len() < MIN_FRAME_SIZE || data.len() > MAX_FRAME_SIZE {
        println!("丢弃帧: 长度异常 caplen={}", data.len());
        return Ok(());
    }
    let dest = &data[..6];
    if dest != &DEST_MAC && dest != &BROADCAST_MAC {
        return Ok(());
    }
    let src = &data[6..12];
    let ether_type = u16::from_be_bytes([data[12], data[13]]);
    let payload_end = data.len() - CRC_LEN;
    let payload = &data[HEADER_LEN..payload_end];
    let crc_expect =
        u32::from_be_bytes(data[payload_end..].try_into().expect("slice len checked"));
    let crc_calc = crc32(&data[..payload_end]);
    if crc_calc != crc_expect {
        println!(
            "丢弃帧: CRC 不匹配 计算={crc_calc:08X} 期望={crc_expect:08X}"
        );
        return Ok(());
    }
    println!(
        "收到帧: len={} 源MAC={} 目的MAC={} 协议类型=0x{ether_type:04X} CRC32={crc_expect:08X} (校验通过)",
        data.len(),
        fmt_mac(src),
        fmt_mac(dest)
    );
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(OUTPUT_FILE)
        .with_context(|| format!("写入 {OUTPUT_FILE} 失败"))?
        .write_all(payload)
        .with_context(|| format!("追加 {OUTPUT_FILE} 失败"))?;
    println!("已将载荷 {} 字节追加写入 {OUTPUT_FILE}", payload.len());
    Ok(())
}