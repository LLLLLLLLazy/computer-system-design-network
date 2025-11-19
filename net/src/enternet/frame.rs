pub const DEST_MAC: [u8; 6] = [0x33; 6];
pub const SRC_MAC: [u8; 6] = [0x22; 6];
pub const BROADCAST_MAC: [u8; 6] = [0xFF; 6];
pub const ETHER_TYPE_IPV4: u16 = 0x0800;
pub const MIN_PAYLOAD_LEN: usize = 46;
pub const MAX_PAYLOAD_LEN: usize = 1500;
pub const MIN_FRAME_SIZE: usize = 64;
pub const MAX_FRAME_SIZE: usize = 1518;
pub const HEADER_LEN: usize = 14;
pub const CRC_LEN: usize = 4;
pub const INPUT_FILE: &str = "data/input_file.txt";
pub const OUTPUT_FILE: &str = "data/output_file.txt";

pub fn fmt_mac(mac: &[u8]) -> String {
    mac.iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(":")
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