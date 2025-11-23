use anyhow::{Result, anyhow};
use std::{
    collections::{HashMap, hash_map::Entry},
    sync::atomic::{AtomicU16, Ordering},
    time::{Duration, Instant},
};

const IPV4_VERSION: u8 = 4;
const BASE_HEADER_LEN: usize = 20;
const OPTIONS_LEN: usize = 40;
const FIXED_HEADER_LEN: usize = BASE_HEADER_LEN + OPTIONS_LEN;
const MAX_TOTAL_LENGTH: usize = 65_535;
const MAX_FRAGMENT_PAYLOAD: usize = 1_400;
const FRAGMENT_TIMEOUT: Duration = Duration::from_secs(30);

static IDENT_COUNTER: AtomicU16 = AtomicU16::new(0);

#[derive(Debug, Clone)]
pub struct Ipv4Header {
    pub version: u8,
    pub ihl: u8,
    pub tos: u8,
    pub total_length: u16,
    pub identification: u16,
    pub df: bool,
    pub mf: bool,
    pub fragment_offset: u16,
    pub ttl: u8,
    pub protocol: u8,
    pub checksum: u16,
    pub src: [u8; 4],
    pub dst: [u8; 4],
}

impl Ipv4Header {
    pub fn header_len_bytes(&self) -> usize {
        (self.ihl as usize) * 4
    }

    pub fn fragment_offset_bytes(&self) -> usize {
        (self.fragment_offset as usize) * 8
    }
}

#[derive(Debug, Clone)]
pub struct Ipv4BuildParams {
    pub src: [u8; 4],
    pub dst: [u8; 4],
    pub protocol: u8,
    pub ttl: u8,
    pub tos: u8,
    pub df: bool,
    pub identification: Option<u16>,
}

#[derive(Debug)]
pub struct Ipv4Packet {
    pub header: Ipv4Header,
    pub bytes: Vec<u8>,
}

#[derive(Debug)]
pub struct ParsedIpv4<'a> {
    pub header: Ipv4Header,
    pub options: &'a [u8],
    pub payload: &'a [u8],
}

#[derive(Debug, Clone)]
pub struct ReassembledPacket {
    pub header: Ipv4Header,
    pub payload: Vec<u8>,
}

pub struct Ipv4Reassembler {
    buffers: HashMap<ReassemblyKey, FragmentBuffer>,
    timeout: Duration,
}

impl Ipv4Reassembler {
    pub fn new() -> Self {
        Self {
            buffers: HashMap::new(),
            timeout: FRAGMENT_TIMEOUT,
        }
    }

    pub fn push_fragment(
        &mut self,
        header: Ipv4Header,
        payload: &[u8],
    ) -> Option<ReassembledPacket> {
        if header.fragment_offset == 0 && !header.mf {
            return Some(ReassembledPacket {
                header,
                payload: payload.to_vec(),
            });
        }
        let key = ReassemblyKey::new(&header);
        match self.buffers.entry(key) {
            Entry::Occupied(mut occupied) => {
                let buffer = occupied.get_mut();
                if header.fragment_offset == 0 {
                    buffer.header = header.clone();
                }
                let offset = header.fragment_offset_bytes();
                if buffer.fragments.iter().any(|frag| frag.offset == offset) {
                    return None;
                }
                buffer.fragments.push(Fragment {
                    offset,
                    data: payload.to_vec(),
                });
                if !header.mf {
                    buffer.total_payload_len = Some(offset + payload.len());
                }
                if let Some(packet) = buffer.try_assemble() {
                    occupied.remove_entry();
                    Some(packet)
                } else {
                    None
                }
            }
            Entry::Vacant(vacant) => {
                let mut buffer = FragmentBuffer::new(header.clone());
                let offset = header.fragment_offset_bytes();
                buffer.fragments.push(Fragment {
                    offset,
                    data: payload.to_vec(),
                });
                if !header.mf {
                    buffer.total_payload_len = Some(offset + payload.len());
                }
                if let Some(packet) = buffer.try_assemble() {
                    Some(packet)
                } else {
                    vacant.insert(buffer);
                    None
                }
            }
        }
    }

    pub fn remove_expired(&mut self) -> Vec<Ipv4Header> {
        let timeout = self.timeout;
        let mut expired = Vec::new();
        self.buffers.retain(|_, buffer| {
            if buffer.start.elapsed() >= timeout {
                expired.push(buffer.header.clone());
                false
            } else {
                true
            }
        });
        expired
    }
}

struct FragmentBuffer {
    start: Instant,
    header: Ipv4Header,
    fragments: Vec<Fragment>,
    total_payload_len: Option<usize>,
}

impl FragmentBuffer {
    fn new(header: Ipv4Header) -> Self {
        Self {
            start: Instant::now(),
            header,
            fragments: Vec::new(),
            total_payload_len: None,
        }
    }

    fn try_assemble(&mut self) -> Option<ReassembledPacket> {
        let total = self.total_payload_len?;
        if total == 0 {
            let mut header = self.header.clone();
            header.mf = false;
            header.fragment_offset = 0;
            header.total_length = header.header_len_bytes() as u16;
            header.checksum = 0;
            return Some(ReassembledPacket {
                header,
                payload: Vec::new(),
            });
        }
        self.fragments.sort_by_key(|frag| frag.offset);
        let mut cursor = 0usize;
        for frag in &self.fragments {
            if frag.offset > cursor {
                return None;
            }
            cursor = cursor.max(frag.offset + frag.data.len());
        }
        if cursor != total {
            return None;
        }
        let mut payload = vec![0u8; total];
        for frag in &self.fragments {
            let end = frag.offset + frag.data.len();
            payload[frag.offset..end].copy_from_slice(&frag.data);
        }
        let mut header = self.header.clone();
        header.mf = false;
        header.fragment_offset = 0;
        header.total_length = header.header_len_bytes() as u16 + total as u16;
        header.checksum = 0;
        Some(ReassembledPacket { header, payload })
    }
}

struct Fragment {
    offset: usize,
    data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ReassemblyKey {
    src: [u8; 4],
    dst: [u8; 4],
    identification: u16,
    protocol: u8,
}

impl ReassemblyKey {
    fn new(header: &Ipv4Header) -> Self {
        Self {
            src: header.src,
            dst: header.dst,
            identification: header.identification,
            protocol: header.protocol,
        }
    }
}

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

pub fn ipv4_checksum(header: &[u8]) -> u16 {
    let mut sum: u32 = 0;
    let mut chunks = header.chunks_exact(2);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn base_params() -> Ipv4BuildParams {
        Ipv4BuildParams {
            src: [10, 0, 0, 1],
            dst: [10, 0, 0, 2],
            protocol: 17,
            ttl: 64,
            tos: 0,
            df: false,
            identification: Some(0x1234),
        }
    }

    #[test]
    fn build_and_parse_single_fragment() -> Result<()> {
        let payload = b"hello ipv4".as_ref();
        let packets = build_ipv4_packets(payload, &base_params())?;
        assert_eq!(packets.len(), 1);
        let parsed = parse_ipv4_packet(&packets[0].bytes)?;
        assert_eq!(parsed.payload, payload);
        Ok(())
    }

    #[test]
    fn reassemble_two_fragments() -> Result<()> {
        let payload = vec![0xAB; MAX_FRAGMENT_PAYLOAD + 100];
        let packets = build_ipv4_packets(&payload, &base_params())?;
        assert_eq!(packets.len(), 2);

        let mut reassembler = Ipv4Reassembler::new();
        let mut assembled = None;
        for pkt in packets {
            let out = reassembler.push_fragment(
                pkt.header.clone(),
                &pkt.bytes[pkt.header.header_len_bytes()..],
            );
            if out.is_some() {
                assembled = out;
            }
        }
        let result = assembled.expect("fragments should reassemble");
        assert_eq!(result.payload, payload);
        Ok(())
    }
}
