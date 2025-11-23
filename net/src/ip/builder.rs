use anyhow::{Result, anyhow};
use std::sync::atomic::{AtomicU16, Ordering};

use super::{
    checksum::ipv4_checksum,
    common::{
        FIXED_HEADER_LEN, IPV4_VERSION, Ipv4BuildParams, Ipv4Header, Ipv4Packet,
        MAX_FRAGMENT_PAYLOAD, MAX_TOTAL_LENGTH, OPTIONS_LEN,
    },
};

static IDENT_COUNTER: AtomicU16 = AtomicU16::new(0);

pub fn build_ipv4_packets(payload: &[u8], params: &Ipv4BuildParams) -> Result<Vec<Ipv4Packet>> {
    if params.df && payload.len() > MAX_FRAGMENT_PAYLOAD {
        return Err(anyhow!(
            "DF=1 不允许分片，但载荷 {} 字节超过阈值 {}",
            payload.len(),
            MAX_FRAGMENT_PAYLOAD
        ));
    }

    let ihl = (FIXED_HEADER_LEN / 4) as u8;
    let identification = params
        .identification
        .unwrap_or_else(|| IDENT_COUNTER.fetch_add(1, Ordering::Relaxed));

    if payload.is_empty() {
        return Ok(vec![build_single_fragment(
            &[],
            params,
            identification,
            ihl,
            false,
            0,
        )?]);
    }

    let mut fragments = Vec::new();
    let mut offset = 0usize;

    while offset < payload.len() {
        let remaining = payload.len() - offset;
        let mut current = remaining.min(MAX_FRAGMENT_PAYLOAD);
        let is_last = current == remaining;

        if !is_last {
            current &= !0x7;
            if current == 0 {
                return Err(anyhow!("无法按 8 字节对齐分片大小"));
            }
        }

        let slice = &payload[offset..offset + current];
        let fragment_offset = (offset / 8) as u16;
        let mf = !is_last;

        fragments.push(build_single_fragment(
            slice,
            params,
            identification,
            ihl,
            mf,
            fragment_offset,
        )?);

        offset += current;
    }

    Ok(fragments)
}

fn build_single_fragment(
    data: &[u8],
    params: &Ipv4BuildParams,
    identification: u16,
    ihl: u8,
    mf: bool,
    fragment_offset: u16,
) -> Result<Ipv4Packet> {
    let header_len = (ihl as usize) * 4;
    let total_length = header_len + data.len();

    if total_length > MAX_TOTAL_LENGTH {
        return Err(anyhow!("IPv4 分片长度超出 65535 字节上限"));
    }

    let mut header_bytes = Vec::with_capacity(header_len);
    header_bytes.push((IPV4_VERSION << 4) | ihl);
    header_bytes.push(params.tos);
    header_bytes.extend_from_slice(&(total_length as u16).to_be_bytes());
    header_bytes.extend_from_slice(&identification.to_be_bytes());

    let flags_fragment =
        ((params.df as u16) << 14) | ((mf as u16) << 13) | (fragment_offset & 0x1FFF);
    header_bytes.extend_from_slice(&flags_fragment.to_be_bytes());
    header_bytes.push(params.ttl);
    header_bytes.push(params.protocol);
    header_bytes.extend_from_slice(&0u16.to_be_bytes());
    header_bytes.extend_from_slice(&params.src);
    header_bytes.extend_from_slice(&params.dst);
    header_bytes.extend_from_slice(&[0u8; OPTIONS_LEN]);

    let checksum = ipv4_checksum(&header_bytes);
    header_bytes[10..12].copy_from_slice(&checksum.to_be_bytes());

    let mut bytes = header_bytes.clone();
    bytes.extend_from_slice(data);

    let header = Ipv4Header {
        version: IPV4_VERSION,
        ihl,
        tos: params.tos,
        total_length: total_length as u16,
        identification,
        df: params.df,
        mf,
        fragment_offset,
        ttl: params.ttl,
        protocol: params.protocol,
        checksum,
        src: params.src,
        dst: params.dst,
    };

    Ok(Ipv4Packet { header, bytes })
}
