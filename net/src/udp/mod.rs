mod recv;
mod send;
mod socket;
mod types;

pub use recv::handle_udp_packet;
pub use send::sendto;
pub use socket::{bind, closesocket, recvfrom, socket};
pub use types::{SocketId, SockAddrIn, AF_INET, IPPROTO_IP, IPPROTO_UDP, INVALID_SOCKET, SOCK_DGRAM};

#[cfg(test)]
mod test;
