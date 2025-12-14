use std::{
	collections::{HashMap, VecDeque},
	env,
	sync::{Arc, Condvar, Mutex, OnceLock, RwLock},
	sync::atomic::{AtomicU16, AtomicU32, Ordering},
};

use anyhow::{Context, Result, anyhow};
use pcap::Device;

use crate::enternet::net::{iface_ipv4, iface_mac};

use super::types::{SocketId, SockAddrIn, AF_INET, IPPROTO_IP, IPPROTO_UDP, SOCK_DGRAM};

#[derive(Debug)]
pub(crate) struct UdpSocket {
	id: SocketId,
	iface: String,
	local_mac: [u8; 6],
	inner: Mutex<SocketInner>,
	recv_cv: Condvar,
}

#[derive(Debug)]
struct SocketInner {
	local_ip: [u8; 4],
	local_port: u16,
	remote: Option<SockAddrIn>,
	queue: VecDeque<Incoming>,
	closed: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct Incoming {
	pub(crate) data: Vec<u8>,
	pub(crate) src: SockAddrIn,
}

impl UdpSocket {
	pub(crate) fn new(
		id: SocketId,
		iface: String,
		local_ip: [u8; 4],
		local_mac: [u8; 6],
		local_port: u16,
	) -> Self {
		Self {
			id,
			iface,
			local_mac,
			inner: Mutex::new(SocketInner {
				local_ip,
				local_port,
				remote: None,
				queue: VecDeque::new(),
				closed: false,
			}),
			recv_cv: Condvar::new(),
		}
	}

	pub(crate) fn set_remote(&self, addr: SockAddrIn) {
		let mut inner = self.inner.lock().unwrap();
		inner.remote = Some(addr);
	}

	pub(crate) fn bind(&self, addr: SockAddrIn) {
		let mut inner = self.inner.lock().unwrap();
		inner.local_port = addr.port;
		inner.local_ip = addr.ip;
	}

	pub(crate) fn local_info(&self) -> ([u8; 4], u16) {
		let inner = self.inner.lock().unwrap();
		(inner.local_ip, inner.local_port)
	}

	pub(crate) fn iface(&self) -> &str {
		&self.iface
	}

	pub(crate) fn local_mac(&self) -> [u8; 6] {
		self.local_mac
	}

	pub(crate) fn enqueue(&self, packet: Incoming) {
		let mut inner = self.inner.lock().unwrap();
		if inner.closed {
			return;
		}
		inner.queue.push_back(packet);
		self.recv_cv.notify_one();
	}

	pub(crate) fn recv_blocking(&self, buf: &mut [u8]) -> Result<(usize, SockAddrIn)> {
		let mut inner = self.inner.lock().unwrap();
		loop {
			if let Some(pkt) = inner.queue.pop_front() {
				if pkt.data.len() > buf.len() {
					return Err(anyhow!(
						"接收缓冲过小: 需要 {} 字节，实际 {} 字节",
						pkt.data.len(),
						buf.len()
					));
				}
				let n = pkt.data.len();
				buf[..n].copy_from_slice(&pkt.data);
				return Ok((n, pkt.src));
			}
			if inner.closed {
				return Err(anyhow!("socket 已关闭"));
			}
			inner = self.recv_cv.wait(inner).unwrap();
		}
	}

	pub(crate) fn matches(&self, dst_ip: &[u8; 4], dst_port: u16) -> bool {
		let inner = self.inner.lock().unwrap();
		(inner.local_port == dst_port)
			&& (inner.local_ip == *dst_ip || inner.local_ip == [0, 0, 0, 0])
	}

	pub(crate) fn close(&self) {
		let mut inner = self.inner.lock().unwrap();
		inner.closed = true;
		inner.queue.clear();
		self.recv_cv.notify_all();
	}
}

static SOCKETS: OnceLock<RwLock<HashMap<SocketId, Arc<UdpSocket>>>> = OnceLock::new();
static NEXT_SOCKET_ID: AtomicU32 = AtomicU32::new(1);
static EPHEMERAL_PORT: AtomicU16 = AtomicU16::new(49_152);

fn socket_table() -> &'static RwLock<HashMap<SocketId, Arc<UdpSocket>>> {
	SOCKETS.get_or_init(|| RwLock::new(HashMap::new()))
}

fn next_socket_id() -> SocketId {
	NEXT_SOCKET_ID.fetch_add(1, Ordering::Relaxed)
}

fn next_ephemeral_port() -> u16 {
	EPHEMERAL_PORT.fetch_add(1, Ordering::Relaxed)
}

fn choose_iface() -> Result<String> {
	if let Ok(name) = env::var("NET_IFACE") {
		return Ok(name);
	}
	let devices = Device::list().context("pcap_findalldevs 失败")?;
	devices
		.first()
		.map(|d| d.name.clone())
		.ok_or_else(|| anyhow!("未发现可用网卡，请设置环境变量 NET_IFACE"))
}

fn create_socket_on_iface(iface: &str) -> Result<SocketId> {
	let local_ip = iface_ipv4(iface)?;
	let local_mac = iface_mac(iface)?;
	let port = next_ephemeral_port();

	let id = next_socket_id();
	let sock = Arc::new(UdpSocket::new(id, iface.to_string(), local_ip, local_mac, port));
	socket_table().write().unwrap().insert(id, Arc::clone(&sock));

	println!(
		"[UDP] 新建 socket id={} 本地IP={} 本地端口={} iface={}",
		id,
		crate::enternet::frame::fmt_ipv4(&local_ip),
		port,
		iface
	);

	Ok(id)
}

pub(crate) fn get_socket(id: SocketId) -> Result<Arc<UdpSocket>> {
	let table = socket_table().read().unwrap();
	table
		.get(&id)
		.cloned()
		.ok_or_else(|| anyhow!("无效的 socket id {id}"))
}

pub(crate) fn find_socket(dst_ip: &[u8; 4], dst_port: u16) -> Option<Arc<UdpSocket>> {
	let table = socket_table().read().unwrap();
	let found = table
		.values()
		.find(|sock| sock.matches(dst_ip, dst_port))
		.cloned();
	drop(table);
	found
}

pub fn socket(af: i32, sock_type: i32, protocol: i32) -> Result<SocketId> {
	if af != AF_INET {
		return Err(anyhow!("仅支持 AF_INET"));
	}
	if sock_type != SOCK_DGRAM {
		return Err(anyhow!("仅支持 SOCK_DGRAM"));
	}
	if protocol != IPPROTO_IP && protocol != IPPROTO_UDP as i32 {
		return Err(anyhow!("protocol 仅支持 IPPROTO_IP 或 UDP"));
	}

	let iface = choose_iface()?;
	create_socket_on_iface(&iface)
}

pub fn socket_on_iface(iface: &str) -> Result<SocketId> {
	create_socket_on_iface(iface)
}

pub fn bind(id: SocketId, addr: SockAddrIn) -> Result<()> {
	let sock = get_socket(id)?;
	sock.bind(addr);
	println!(
		"[UDP] socket id={} 绑定到 {}:{}",
		id,
		crate::enternet::frame::fmt_ipv4(&addr.ip),
		addr.port
	);
	Ok(())
}

pub fn recvfrom(id: SocketId, buf: &mut [u8], _flags: u32) -> Result<(usize, SockAddrIn)> {
	let sock = get_socket(id)?;
	sock.recv_blocking(buf)
}

pub fn closesocket(id: SocketId) -> Result<()> {
	let sock = {
		let mut table = socket_table().write().unwrap();
		match table.remove(&id) {
			Some(sock) => sock,
			None => return Err(anyhow!("无效的 socket id {id}")),
		}
	};
	sock.close();
	println!("[UDP] socket id={} 已关闭", id);
	Ok(())
}

pub(crate) fn enqueue(id: SocketId, packet: Incoming) -> Result<()> {
	let sock = get_socket(id)?;
	sock.enqueue(packet);
	Ok(())
}

pub(crate) fn sockets_iter() -> Vec<Arc<UdpSocket>> {
	let table = socket_table().read().unwrap();
	table.values().cloned().collect()
}

#[cfg(test)]
pub(crate) fn register_test_socket(
	iface: String,
	local_ip: [u8; 4],
	local_mac: [u8; 6],
	local_port: u16,
) -> (SocketId, Arc<UdpSocket>) {
	let id = next_socket_id();
	let sock = Arc::new(UdpSocket::new(id, iface, local_ip, local_mac, local_port));
	socket_table().write().unwrap().insert(id, sock.clone());
	(id, sock)
}

#[cfg(test)]
pub(crate) fn remove_socket(id: SocketId) {
	socket_table().write().unwrap().remove(&id);
}