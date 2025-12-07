#![allow(unused)]

mod api;

use super::{bind, closesocket, handle_udp_packet, recvfrom, socket, SockAddrIn, AF_INET, IPPROTO_IP, SOCK_DGRAM};
use super::types::UDP_HEADER_LEN;
use super::send::udp_checksum;
use crate::ip::Ipv4Header;

// 测试原理：
// 1) 构造完整的 UDP 段，填入伪首部校验和，配合 IPv4 头部模拟真实入站报文。
// 2) 通过 handle_udp_packet 走正常校验、端口匹配、入队逻辑，然后用 recvfrom 取出。
// 3) 保持日志输出，便于在终端观察收发细节（cargo test -- --nocapture / --ignored）。

use std::sync::Arc;

// 说明：此测试为手动/示例测试，依赖本机 pcap 与 NET_IFACE 环境，默认忽略。
#[test]
#[ignore]
fn inject_udp_payload_to_socket_queue() {
	let sock = socket(AF_INET, SOCK_DGRAM, IPPROTO_IP).expect("create socket");
	let local_addr = SockAddrIn {
		ip: [0, 0, 0, 0],
		port: 50_500,
	};
	bind(sock, local_addr).expect("bind");

	// 构造伪 IPv4 + UDP 报文并直接注入 handle_udp_packet（无需实际发包）。
	let src_ip = [10, 1, 1, 1];
	let dst_ip = [192, 168, 1, 123];
	let src_port = 60_000u16;
	let dst_port = local_addr.port;
	let payload = b"hello-udp";
	let udp_len = UDP_HEADER_LEN + payload.len();

	let mut segment = Vec::with_capacity(udp_len);
	segment.extend_from_slice(&src_port.to_be_bytes());
	segment.extend_from_slice(&dst_port.to_be_bytes());
	segment.extend_from_slice(&(udp_len as u16).to_be_bytes());
	segment.extend_from_slice(&0u16.to_be_bytes());
	segment.extend_from_slice(payload);

	let csum = udp_checksum(&src_ip, &dst_ip, &segment);
	segment[6..8].copy_from_slice(&csum.to_be_bytes());

	let ipv4 = Ipv4Header {
		version: 4,
		ihl: 5,
		tos: 0,
		total_length: (20 + udp_len) as u16,
		identification: 0,
		df: false,
		mf: false,
		fragment_offset: 0,
		ttl: 64,
		protocol: super::types::IPPROTO_UDP,
		checksum: 0,
		src: src_ip,
		dst: dst_ip,
	};

	handle_udp_packet(&ipv4, &segment).expect("udp handle");

	let mut buf = [0u8; 64];
	let (n, src) = recvfrom(sock, &mut buf, 0).expect("recv");
	assert_eq!(&buf[..n], payload);
	assert_eq!(src.ip, src_ip);
	assert_eq!(src.port, src_port);

	closesocket(sock).ok();
}

#[test]
fn simulate_udp_delivery_like_real_world() {
	// 在测试内直接注册 socket，避免依赖 pcap/网卡，仍然走真实的 UDP 校验和与匹配逻辑。
	let local_ip = [192, 168, 56, 10];
	let local_port = 50_501u16;
	let (sock_id, sock) = super::socket::register_test_socket(
		"lo".to_string(), // 名称仅用于日志，不访问系统设备
		local_ip,
		[0x02, 0x00, 0x00, 0x00, 0x00, 0x01],
		local_port,
	);

	let remote_ip = [192, 168, 56, 99];
	let remote_port = 60_001u16;
	let payload = b"realistic-udp-payload";
	let udp_len = UDP_HEADER_LEN + payload.len();

	let mut segment = Vec::with_capacity(udp_len);
	segment.extend_from_slice(&remote_port.to_be_bytes());
	segment.extend_from_slice(&local_port.to_be_bytes());
	segment.extend_from_slice(&(udp_len as u16).to_be_bytes());
	segment.extend_from_slice(&0u16.to_be_bytes());
	segment.extend_from_slice(payload);
	let csum = udp_checksum(&remote_ip, &local_ip, &segment);
	segment[6..8].copy_from_slice(&csum.to_be_bytes());

	let ipv4 = Ipv4Header {
		version: 4,
		ihl: 5,
		tos: 0,
		total_length: (20 + udp_len) as u16,
		identification: 0x1234,
		df: true,
		mf: false,
		fragment_offset: 0,
		ttl: 64,
		protocol: super::types::IPPROTO_UDP,
		checksum: 0,
		src: remote_ip,
		dst: local_ip,
	};

	println!(
		"[TEST] 模拟入站 UDP: {}:{} -> {}:{} 长度={}B",
		crate::enternet::frame::fmt_ipv4(&remote_ip),
		remote_port,
		crate::enternet::frame::fmt_ipv4(&local_ip),
		local_port,
		payload.len()
	);

	handle_udp_packet(&ipv4, &segment).expect("udp handle");

	let mut buf = [0u8; 128];
	let (n, src) = recvfrom(sock_id, &mut buf, 0).expect("recv");
	assert_eq!(&buf[..n], payload);
	assert_eq!(src.ip, remote_ip);
	assert_eq!(src.port, remote_port);

	closesocket(sock_id).ok();
	drop(sock); // 显式释放 Arc，避免影响后续测试
}