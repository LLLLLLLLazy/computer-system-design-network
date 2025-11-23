//// filepath: /Users/lazy/code/network/net/src/ip/parser.rs
use anyhow::{Result, anyhow};
use std::convert::TryInto;

use super::{
    checksum::ipv4_checksum,
    common::{BASE_HEADER_LEN, IPV4_VERSION, Ipv4Header, ParsedIpv4},
};

pub fn parse_ipv4_packet(data: &[u8]) -> Result<ParsedIpv4<'_>> {
    if data.len() < BASE_HEADER_LEN {
        return Err(anyhow!("IPv4 报文过短"));
    }

    let version = data[0] >> 4;
    if version != IPV4_VERSION {
        return Err(anyhow!("仅支持 IPv4 报文，收到版本 {version}"));
    }

    let ihl = data[0] & 0x0F;
    if ihl < 5 {
        return Err(anyhow!("IPv4 IHL 非法: {ihl}"));
    }

    let header_len = (ihl as usize) * 4;
    if data.len() < header_len {
        return Err(anyhow!("IPv4 报文长度不足以包含首部"));
    }

    let total_length = u16::from_be_bytes([data[2], data[3]]) as usize;
    if total_length < header_len {
        return Err(anyhow!("IPv4 总长度小于首部长度"));
    }
    if data.len() < total_length {
        return Err(anyhow!("IPv4 报文长度不完整"));
    }

    let flags_fragment = u16::from_be_bytes([data[6], data[7]]);
    let df = (flags_fragment & 0x4000) != 0;
    let mf = (flags_fragment & 0x2000) != 0;
    let fragment_offset = flags_fragment & 0x1FFF;
    let checksum = u16::from_be_bytes([data[10], data[11]]);

    let mut header_copy = data[..header_len].to_vec();
    header_copy[10] = 0;
    header_copy[11] = 0;
    let calc = ipv4_checksum(&header_copy);
    if calc != checksum {
        return Err(anyhow!(
            "IPv4 校验和错误: 计算={calc:04X} 期望={checksum:04X}"
        ));
    }

    let header = Ipv4Header {
        version,
        ihl,
        tos: data[1],
        total_length: total_length as u16,
        identification: u16::from_be_bytes([data[4], data[5]]),
        df,
        mf,
        fragment_offset,
        ttl: data[8],
        protocol: data[9],
        checksum,
        src: data[12..16].try_into().unwrap(),
        dst: data[16..20].try_into().unwrap(),
    };

    let options = &data[BASE_HEADER_LEN..header_len];
    let payload = &data[header_len..total_length];

    Ok(ParsedIpv4 {
        header,
        options,
        payload,
    })
}
