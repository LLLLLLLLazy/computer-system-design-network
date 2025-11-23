#![cfg(test)]

mod ipv4 {
    use crate::ip::{
        Ipv4BuildParams, Ipv4Reassembler, MAX_FRAGMENT_PAYLOAD, build_ipv4_packets,
        parse_ipv4_packet,
    };
    use anyhow::Result;

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
            let header = pkt.header.clone();
            let payload_slice = &pkt.bytes[pkt.header.header_len_bytes()..];
            if let Some(packet) = reassembler.push_fragment(header, payload_slice) {
                assembled = Some(packet);
            }
        }

        let result = assembled.expect("fragments should reassemble");
        assert_eq!(result.payload, payload);
        Ok(())
    }
}
