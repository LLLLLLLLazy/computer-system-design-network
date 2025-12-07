pub type SocketId = u32;

pub const INVALID_SOCKET: SocketId = 0;
pub const AF_INET: i32 = 2;
pub const SOCK_DGRAM: i32 = 2;
pub const IPPROTO_IP: i32 = 0;
pub const IPPROTO_UDP: u8 = 17;

pub const UDP_HEADER_LEN: usize = 8;
pub const DEFAULT_TTL: u8 = 64;
pub const DEFAULT_TOS: u8 = 0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SockAddrIn {
	pub ip: [u8; 4],
	pub port: u16,
}