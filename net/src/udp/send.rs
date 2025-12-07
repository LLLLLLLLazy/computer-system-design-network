use anyhow::{Context, Result, anyhow};

use crate::{
	config::network_profile,
	enternet::{
		arp::{self, same_subnet},
		datalink::open_device,
		frame::{build_frame, fmt_ipv4, ETHER_TYPE_IPV4},
	},
	ip::Ipv4BuildParams,
};

use super::{
	socket::{get_socket, UdpSocket},
	types::{SockAddrIn, DEFAULT_TOS, DEFAULT_TTL, IPPROTO_UDP, UDP_HEADER_LEN},
};

pub fn sendto(id: super::types::SocketId, buf: &[u8], _flags: u32, dest: SockAddrIn) -> Result<usize> {
	let sock = get_socket(id)?;
	sock.set_remote(dest);
	send_with_socket(&sock, buf, dest)
}

fn send_with_socket(sock: &UdpSocket, payload: &[u8], dest: SockAddrIn) -> Result<usize> {
	if payload.is_empty() {
		return Err(anyhow!("待发送数据为空"));
	}

	let (local_ip, local_port) = sock.local_info();

	let udp_len = UDP_HEADER_LEN + payload.len();
	if udp_len > u16::MAX as usize {
		return Err(anyhow!("UDP 报文过长"));
	}

	let mut segment = Vec::with_capacity(udp_len);
	segment.extend_from_slice(&local_port.to_be_bytes());
	segment.extend_from_slice(&dest.port.to_be_bytes());
	segment.extend_from_slice(&(udp_len as u16).to_be_bytes());
	segment.extend_from_slice(&0u16.to_be_bytes());
	segment.extend_from_slice(payload);

	let checksum = udp_checksum(&local_ip, &dest.ip, &segment);
	segment[6..8].copy_from_slice(&checksum.to_be_bytes());

	let params = Ipv4BuildParams {
		src: local_ip,
		dst: dest.ip,
		protocol: IPPROTO_UDP,
		ttl: DEFAULT_TTL,
		tos: DEFAULT_TOS,
		df: false,
		identification: None,
	};

	let packets = crate::ip::build_ipv4_packets(&segment, &params)?;
	let profile = network_profile();
	let next_hop = if same_subnet(&local_ip, &dest.ip, &profile.subnet_mask) {
		dest.ip
	} else {
		profile.gateway_ip
	};

	let dest_mac = arp::resolve_mac(sock.iface(), sock.local_mac(), local_ip, next_hop)?;
	let mut handle = open_device(sock.iface())?;

	for fragment in packets {
		let frame = build_frame(&dest_mac, &sock.local_mac(), ETHER_TYPE_IPV4, &fragment.bytes);
		handle
			.sendpacket(frame.as_slice())
			.with_context(|| format!("发送 UDP 分片失败 (iface={})", sock.iface()))?;
		println!(
			"[UDP] 已发送: 源 {}:{} -> 目的 {}:{} 分片长={}B",
			fmt_ipv4(&local_ip),
			local_port,
			fmt_ipv4(&dest.ip),
			dest.port,
			frame.len()
		);
	}

	Ok(payload.len())
}

pub(crate) fn udp_checksum(src_ip: &[u8; 4], dst_ip: &[u8; 4], segment: &[u8]) -> u16 {
	let mut sum = 0u32;

	let pseudo_len = segment.len() as u16;
	sum = sum.wrapping_add(u16::from_be_bytes([src_ip[0], src_ip[1]]) as u32);
	sum = sum.wrapping_add(u16::from_be_bytes([src_ip[2], src_ip[3]]) as u32);
	sum = sum.wrapping_add(u16::from_be_bytes([dst_ip[0], dst_ip[1]]) as u32);
	sum = sum.wrapping_add(u16::from_be_bytes([dst_ip[2], dst_ip[3]]) as u32);
	sum = sum.wrapping_add(super::types::IPPROTO_UDP as u32);
	sum = sum.wrapping_add(pseudo_len as u32);

	let mut chunks = segment.chunks_exact(2);
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