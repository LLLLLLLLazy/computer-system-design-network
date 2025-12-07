#![allow(unused)]

use super::super::{bind, closesocket, handle_udp_packet, recvfrom, SockAddrIn};
use super::super::send::udp_checksum;
use super::super::socket::{register_test_socket, remove_socket, UdpSocket};
use super::super::types::{IPPROTO_UDP, UDP_HEADER_LEN};
use crate::ip::Ipv4Header;
use crate::enternet::frame::fmt_ipv4;

// 单接口验证：bind 能更新 socket 的本地信息。
#[test]
fn api_bind_updates_local_info() {
	let local_ip = [10, 0, 0, 10];
	let local_port = 40_001u16;
	let (id, sock) = register_test_socket("lo".to_string(), local_ip, [1, 2, 3, 4, 5, 6], 30_000);

	let new_addr = SockAddrIn { ip: local_ip, port: local_port };
	bind(id, new_addr).expect("bind");
	let (ip, port) = sock.local_info();

	println!("[TEST] bind: ip={} port={}", fmt_ipv4(&ip), port);
	assert_eq!(ip, local_ip);
	assert_eq!(port, local_port);

	closesocket(id).ok();
	drop(sock);
}

// 单接口验证：handle_udp_packet + recvfrom 协同收包。
#[test]
fn api_recvfrom_after_handle_udp() {
	let local_ip = [10, 0, 0, 20];
	let local_port = 40_002u16;
	let (id, _sock) = register_test_socket("lo".to_string(), local_ip, [1, 1, 1, 1, 1, 1], local_port);

	let remote_ip = [10, 0, 0, 30];
	let remote_port = 60_002u16;
	let payload = b"api-recv-test";
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
		identification: 0x2222,
		df: true,
		mf: false,
		fragment_offset: 0,
		ttl: 64,
		protocol: IPPROTO_UDP,
		checksum: 0,
		src: remote_ip,
		dst: local_ip,
	};

	println!(
		"[TEST] api recv path: {}:{} -> {}:{} len={}B",
		fmt_ipv4(&remote_ip),
		remote_port,
		fmt_ipv4(&local_ip),
		local_port,
		payload.len()
	);

	handle_udp_packet(&ipv4, &segment).expect("udp handle");

	let mut buf = [0u8; 64];
	let (n, src) = recvfrom(id, &mut buf, 0).expect("recv");
	assert_eq!(&buf[..n], payload);
	assert_eq!(src.ip, remote_ip);
	assert_eq!(src.port, remote_port);

	closesocket(id).ok();
}

// 单接口验证：udp_checksum 与伪首部计算一致。
#[test]
fn api_udp_checksum_matches() {
	let src = [1, 1, 1, 1];
	let dst = [2, 2, 2, 2];
	let payload = b"checksum";
	let udp_len = UDP_HEADER_LEN + payload.len();
	let mut segment = Vec::with_capacity(udp_len);
	segment.extend_from_slice(&1234u16.to_be_bytes());
	segment.extend_from_slice(&4321u16.to_be_bytes());
	segment.extend_from_slice(&(udp_len as u16).to_be_bytes());
	segment.extend_from_slice(&0u16.to_be_bytes());
	segment.extend_from_slice(payload);
	let csum = udp_checksum(&src, &dst, &segment);
	segment[6..8].copy_from_slice(&csum.to_be_bytes());
	let calc = udp_checksum(&src, &dst, &segment);

	println!("[TEST] checksum: calc=0x{calc:04X}");
	assert_eq!(calc, 0);
}

// 单接口验证：closesocket 移除 socket，后续获取应失败。
#[test]
fn api_closesocket_removes_entry() {
	let (id, sock) = register_test_socket("lo".to_string(), [10, 0, 0, 40], [9, 9, 9, 9, 9, 9], 40_003);
	println!("[TEST] closing socket id={}", id);
	closesocket(id).expect("close");
	let res = super::super::socket::get_socket(id);
	assert!(res.is_err(), "socket should be removed after close");
	// 清理：若表中仍残留则移除。
	remove_socket(id);
	drop(sock);
}

// 端到端：模拟服务器与客户端的 socket/bind/sendto/recvfrom/close 流程（无 pcap，纯内存注入报文）。
#[test]
fn api_server_client_roundtrip() {
	let srv_ip = [10, 0, 1, 10];
	let cli_ip = [10, 0, 1, 20];
	let srv_port = 41_000u16;
	let cli_port = 42_000u16;

	// server: socket + bind
	let (srv_id, _srv) = register_test_socket("lo".to_string(), srv_ip, [0xAA, 1, 1, 1, 1, 1], srv_port);
	bind(srv_id, SockAddrIn { ip: srv_ip, port: srv_port }).expect("srv bind");
	println!("[TEST] server ready at {}:{}", fmt_ipv4(&srv_ip), srv_port);

	// client: socket (已含本地信息)
	let (cli_id, _cli) = register_test_socket("lo".to_string(), cli_ip, [0xBB, 2, 2, 2, 2, 2], cli_port);
	println!("[TEST] client ready at {}:{}", fmt_ipv4(&cli_ip), cli_port);

	// client -> server 请求
	let req_payload = b"service-request";
	send_udp_segment(cli_ip, cli_port, srv_ip, srv_port, req_payload);

	let mut buf = [0u8; 128];
	let (n_req, src_req) = recvfrom(srv_id, &mut buf, 0).expect("srv recv request");
	println!("[TEST] server got request from {}:{} len={}", fmt_ipv4(&src_req.ip), src_req.port, n_req);
	assert_eq!(&buf[..n_req], req_payload);
	assert_eq!(src_req.ip, cli_ip);
	assert_eq!(src_req.port, cli_port);

	// server -> client 响应
	let resp_payload = b"service-response";
	send_udp_segment(srv_ip, srv_port, cli_ip, cli_port, resp_payload);

	let mut buf2 = [0u8; 128];
	let (n_resp, src_resp) = recvfrom(cli_id, &mut buf2, 0).expect("cli recv response");
	println!("[TEST] client got response from {}:{} len={} \"{}\"", fmt_ipv4(&src_resp.ip), src_resp.port, n_resp, String::from_utf8_lossy(&buf2[..n_resp]));
	assert_eq!(&buf2[..n_resp], resp_payload);
	assert_eq!(src_resp.ip, srv_ip);
	assert_eq!(src_resp.port, srv_port);

	closesocket(cli_id).ok();
	closesocket(srv_id).ok();
	remove_socket(cli_id);
	remove_socket(srv_id);
}

// helper: 构造 UDP 段 + IPv4 头并通过 handle_udp_packet 注入。
fn send_udp_segment(src_ip: [u8; 4], src_port: u16, dst_ip: [u8; 4], dst_port: u16, payload: &[u8]) {
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
		identification: 0xBEEF,
		df: true,
		mf: false,
		fragment_offset: 0,
		ttl: 64,
		protocol: IPPROTO_UDP,
		checksum: 0,
		src: src_ip,
		dst: dst_ip,
	};

	println!(
		"[TEST] inject: {}:{} -> {}:{} len={}B",
		fmt_ipv4(&src_ip),
		src_port,
		fmt_ipv4(&dst_ip),
		dst_port,
		payload.len()
	);

	handle_udp_packet(&ipv4, &segment).expect("udp handle");
}
