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
