use std::time::Duration;

pub const IPV4_VERSION: u8 = 4;
pub const BASE_HEADER_LEN: usize = 20;
pub const OPTIONS_LEN: usize = 40;
pub const FIXED_HEADER_LEN: usize = BASE_HEADER_LEN + OPTIONS_LEN;
pub const MAX_TOTAL_LENGTH: usize = 65_535;
pub const MAX_FRAGMENT_PAYLOAD: usize = 1_400;
pub const FRAGMENT_TIMEOUT: Duration = Duration::from_secs(30);

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
