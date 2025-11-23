//// filepath: /Users/lazy/code/network/net/src/ip/reassembly.rs
use std::{
    collections::{HashMap, hash_map::Entry},
    time::Instant,
};

use super::common::{FRAGMENT_TIMEOUT, Ipv4Header, ReassembledPacket};

pub struct Ipv4Reassembler {
    buffers: HashMap<ReassemblyKey, FragmentBuffer>,
}

impl Ipv4Reassembler {
    pub fn new() -> Self {
        Self {
            buffers: HashMap::new(),
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
        let mut expired = Vec::new();
        self.buffers.retain(|_, buffer| {
            if buffer.start.elapsed() >= FRAGMENT_TIMEOUT {
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
