//pub const DEST_MAC: [u8; 6] = [0x33; 6];
//pub const SRC_MAC: [u8; 6] = [0x22; 6];
pub const BROADCAST_MAC: [u8; 6] = [0xFF; 6];
pub const ETHER_TYPE_IPV4: u16 = 0x0800;
pub const ETHER_TYPE_ARP: u16 = 0x0806;
//pub const MIN_PAYLOAD_LEN: usize = 46;
//pub const MAX_PAYLOAD_LEN: usize = 1500;
pub const MIN_FRAME_SIZE: usize = 64;
pub const MAX_FRAME_SIZE: usize = 1518;
pub const HEADER_LEN: usize = 14;
pub const CRC_LEN: usize = 4;
pub const INPUT_FILE: &str = "data/input_file.txt";
pub const OUTPUT_FILE: &str = "data/output_file.txt";
pub const IPV4_BROADCAST: [u8; 4] = [255, 255, 255, 255];

pub fn fmt_mac(mac: &[u8]) -> String {
    mac.iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(":")
}

pub fn fmt_ipv4(addr: &[u8; 4]) -> String {
    addr.iter()
        .map(|oct| oct.to_string())
        .collect::<Vec<_>>()
        .join(".")
}

pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for byte in data {
        crc ^= (*byte as u32) << 24;
        for _ in 0..8 {
            crc = if crc & 0x8000_0000 != 0 {
                (crc << 1) ^ 0x04C1_1DB7
            } else {
                crc << 1
            };
        }
    }
    !crc
}

pub fn build_frame(
    dest_mac: &[u8; 6],
    src_mac: &[u8; 6],
    ether_type: u16,
    payload: &[u8],
) -> Vec<u8> {
    let mut frame = Vec::with_capacity(HEADER_LEN + payload.len() + CRC_LEN);
    frame.extend_from_slice(dest_mac);
    frame.extend_from_slice(src_mac);
    frame.extend_from_slice(&ether_type.to_be_bytes());
    frame.extend_from_slice(payload);
    let crc = crc32(&frame);
    frame.extend_from_slice(&crc.to_be_bytes());
    if frame.len() < MIN_FRAME_SIZE {
        frame.resize(MIN_FRAME_SIZE, 0);
    }
    frame
}
