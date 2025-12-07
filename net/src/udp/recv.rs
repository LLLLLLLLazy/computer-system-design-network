use anyhow::Result;

use crate::ip::Ipv4Header;

use super::{
	send::udp_checksum,
	socket::{find_socket, Incoming},
	types::UDP_HEADER_LEN,
};

pub fn handle_udp_packet(ip_header: &Ipv4Header, payload: &[u8]) -> Result<()> {
	if payload.len() < UDP_HEADER_LEN {
		return Ok(());
	}

	let src_port = u16::from_be_bytes([payload[0], payload[1]]);
	let dst_port = u16::from_be_bytes([payload[2], payload[3]]);
	let udp_len = u16::from_be_bytes([payload[4], payload[5]]) as usize;

	if udp_len < UDP_HEADER_LEN || udp_len > payload.len() {
		return Ok(());
	}

	let recv_checksum = u16::from_be_bytes([payload[6], payload[7]]);
	let mut seg = payload[..udp_len].to_vec();
	seg[6] = 0;
	seg[7] = 0;
	let calc = udp_checksum(&ip_header.src, &ip_header.dst, &seg);
	if recv_checksum != 0 && calc != recv_checksum {
		eprintln!(
			"[UDP] 丢弃报文: 校验和错误 calc=0x{calc:04X} expect=0x{recv_checksum:04X}"
		);
		return Ok(());
	}

	let Some(sock) = find_socket(&ip_header.dst, dst_port) else {
		eprintln!(
			"[UDP] 未找到匹配 socket，目的 IP={} 端口={}",
			crate::enternet::frame::fmt_ipv4(&ip_header.dst),
			dst_port
		);
		return Ok(());
	};

	let data = &payload[UDP_HEADER_LEN..udp_len];
	let incoming = Incoming {
		data: data.to_vec(),
		src: super::types::SockAddrIn {
			ip: ip_header.src,
			port: src_port,
		},
	};
	sock.enqueue(incoming);
	println!(
		"[UDP] 已接收: 源 {}:{} -> 本地 {}:{} 数据={}B",
		crate::enternet::frame::fmt_ipv4(&ip_header.src),
		src_port,
		crate::enternet::frame::fmt_ipv4(&ip_header.dst),
		dst_port,
		data.len()
	);
	Ok(())
}