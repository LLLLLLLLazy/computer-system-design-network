# UDP 模块接口说明

本目录实现了简化版 UDP 协议栈，对外暴露 5 个主要接口，位于 `udp/mod.rs` 并复用 IPv4 与数据链路层逻辑。

## 公开接口

- `socket(af, sock_type, protocol) -> SocketId`
  - 仅支持 `AF_INET` + `SOCK_DGRAM`，`protocol` 支持 `IPPROTO_IP`/`IPPROTO_UDP`。
  - 自动选择网卡（`NET_IFACE` 环境变量，或首个可用 pcap 设备），读取本机 IP/MAC，分配临时端口（起始 49152）。
  - 返回的 `SocketId` 是内部维护的 UDP 五元组句柄。

- `bind(sock, SockAddrIn)`
  - 将 socket 绑定到指定本地 IP/端口；IP 可为 0.0.0.0 表示任意。

- `sendto(sock, buf, flags, dest: SockAddrIn) -> usize`
  - 构造 UDP 报文（包含伪首部校验和），利用 IPv4 分片发送。
  - 依据子网掩码决定直连或走网关，ARP 解析下一跳 MAC，走 pcap 发送以太帧。
  - 返回已请求发送的 payload 字节数。

- `recvfrom(sock, buf, flags) -> (usize, SockAddrIn)`
  - 阻塞从对应 socket 队列取出数据，复制到 `buf`，返回实际字节数与源地址。

- `closesocket(sock) -> ()`
  - 关闭 socket，释放内部队列并唤醒等待的接收方。

## 实现细节说明（结合源码）

本文档补充解释五个公开接口在本仓库中的具体实现路径与核心逻辑，帮助理解 UDP 模块如何与 IPv4、ARP 与数据链路层协作。

### 1) socket(af, sock_type, protocol) -> SocketId

入口：`udp/mod.rs` 转发到 `udp/socket.rs`。

- 路径与符号
  - `udp/mod.rs` 中的 `pub fn socket(...)`：调用 `socket::socket_on_iface(...)`。
  - `udp/socket.rs`：
    - `pub fn socket_on_iface(af: i32, sock_type: i32, protocol: i32) -> Result<SocketId>`
    - `struct UdpSocket`：内部维护五元组、本地地址/端口、接收队列。
    - 全局注册表：用于按目的 IP/端口匹配 socket。

- 关键流程
  - 参数校验：仅允许 `AF_INET` + `SOCK_DGRAM`，`protocol` 支持 `IPPROTO_IP` 或 `IPPROTO_UDP`（见 `types.rs` 常量）。
  - 网卡选择与本机地址初始化：
    - 从环境变量 `NET_IFACE` 或通过 pcap 获取默认设备与本机 MAC/IP（复用 `enternet/net.rs` 或 `datalink/mod.rs` 的设备打开函数）。
  - 端口分配：从临时端口范围（起始 49152，见 `socket.rs` 中的常量或生成器）分配一个未被占用的本地端口。
  - 构造 `UdpSocket` 并注册到全局表，返回 `SocketId`（句柄）。

- 重要点
  - 资源在“堆”分配，`SocketId` 只是索引/句柄；关闭时释放。
  - 本地 IP 可默认填本机实际地址或 0.0.0.0，后续 `bind` 可覆盖。

### 2) bind(sock, SockAddrIn)

入口：`udp/mod.rs` -> `udp/socket.rs`。

- 路径与符号
  - `udp/mod.rs` 的 `pub fn bind(sock: SocketId, addr: SockAddrIn) -> Result<()>`
  - `udp/socket.rs` 的 `pub fn bind(sock: SocketId, addr: SockAddrIn) -> Result<()>`
  - `udp/types.rs` 的 `SockAddrIn { ip: [u8;4], port: u16 }` 与常量

- 关键流程
  - 通过 `SocketId` 在全局表中查找 `UdpSocket`。
  - 更新 socket 的本地地址与端口：
    - IP 可为 0.0.0.0（表示任意，本模块接收时仅按端口匹配；IP 进一步由上层/路由决定）。
    - 端口为服务端周知端口时需检测冲突，避免重复绑定。
  - 将绑定后的 socket 重新登记到“目的端口匹配索引”，用于接收路径快速查找。

- 重要点
  - 该操作等价于将“服务器进程标识（IP + 端口）”与 socket 关联，供 `recv` 路径匹配。
  - 未绑定时，发送使用临时端口，接收可能无法匹配（除非你在发送前已绑定）。

### 3) sendto(sock, buf, flags, dest: SockAddrIn) -> usize

入口：`udp/mod.rs` -> `udp/send.rs`。

- 路径与符号
  - `udp/mod.rs` 的 `pub fn sendto(...) -> Result<usize>`
  - `udp/send.rs` 的 `pub fn sendto(sock: SocketId, buf: &[u8], flags: u32, dest: SockAddrIn) -> Result<usize>`
  - `udp/send.rs` 的 `fn build_segment(...) -> Vec<u8>` 与 `fn udp_checksum(...) -> u16`
  - 依赖：
    - IPv4 分片构造：`ip/builder.rs::build_ipv4_packets(...)`
    - 同/异网段判断与 ARP：`enternet/arp.rs::same_subnet(...)`、`resolve_mac(...)`
    - 发帧：`enternet/frame.rs::build_frame(...)` + `datalink/send.rs::send_frame(...)`

- 关键流程
  1. 从 `SocketId` 取到本地 `UdpSocket`，准备本地 IP/端口与目的 IP/端口（dest）。
  2. 构造 UDP 首部（源端口、目的端口、长度、校验和占位 0）。
  3. 构造伪首部（source IP、dest IP、协议号=17、UDP 长度），计算校验和（包含伪首部 + UDP 首部 + 数据），写回 UDP 首部校验和字段。
  4. 将完整 UDP 段作为 IPv4 载荷，调用 `build_ipv4_packets` 进行分片（根据 MTU/阈值）。
  5. 路由决策：`same_subnet(src_ip, dest_ip, mask)` 决定直连或走网关 IP。
  6. 解析下一跳 MAC：`resolve_mac(iface, src_mac, src_ip, next_hop_ip)`，支持缓存与重试。
  7. 对每个 IPv4 分片，调用 `build_frame(dst_mac, src_mac, EtherType::IPV4, payload)` 生成以太网帧，并通过 pcap 发送。
  8. 返回请求发送的 payload 字节数（通常为 `buf.len()`）。

- 重要点
  - 校验和实现：严格按照“伪首部 + 首部 + 数据”；当 `buf` 为空时也需合法长度与校验。
  - 不做重传与拥塞控制；底层由链路与网卡处理，应用需要自定义可靠性协议则应在 UDP 上层。

### 4) recvfrom(sock, buf, flags) -> (usize, SockAddrIn)

入口：`udp/mod.rs` -> `udp/socket.rs`（阻塞队列取数），入站填充在 `udp/recv.rs`。

- 路径与符号
  - `udp/mod.rs` 的 `pub fn recvfrom(sock: SocketId, buf: &mut [u8], flags: u32) -> Result<(usize, SockAddrIn)>`
  - `udp/socket.rs` 的 `pub fn recvfrom(...) -> Result<(usize, SockAddrIn)>`
  - 入站：
    - `enternet/datalink/recv.rs` 在 IPv4 重组后分派到 UDP
    - `udp/recv.rs` 的 `pub fn handle_udp_packet(ip_header, udp_payload, src_ip)`

- 入站关键流程（handle_udp_packet）
  1. 解析 UDP 首部与数据，校验长度一致性。
  2. 计算并验证校验和（伪首部 + 首部 + 数据）；错误则丢弃并日志。
  3. 在全局 socket 表中按“目的 IP/端口”查找匹配的 `UdpSocket`；若未命中，目前仅打印提示（可扩展为发送 ICMP 端口不可达）。
  4. 命中则封装 `Incoming { data, src: SockAddrIn{ ip: src_ip, port: src_port } }` 入队，唤醒阻塞的接收方。

- 出队关键流程（recvfrom）
  - 从 `UdpSocket` 的接收队列阻塞等待（条件变量/信号）。
  - 将数据复制到用户 `buf`，返回 `(字节数, 源地址)`。

- 重要点
  - 当 `bind` 到特定端口后，入站才可匹配到该 socket。
  - 未实现队列溢出丢弃策略时，建议控制应用层消费速度。

### 5) closesocket(sock) -> ()

入口：`udp/mod.rs` -> `udp/socket.rs`。

- 路径与符号
  - `udp/mod.rs` 的 `pub fn closesocket(sock: SocketId) -> Result<()>`
  - `udp/socket.rs` 的 `pub fn closesocket(sock: SocketId) -> Result<()>`

- 关键流程
  - 从全局表移除该 `UdpSocket` 并释放其队列与内部内存。
  - 唤醒可能阻塞的 `recvfrom` 调用，使其返回错误或空数据，避免死锁。
  - 释放端口占用，供后续分配。

- 重要点
  - 关闭后，句柄失效；进一步调用发送/接收将返回错误。
  - 清理时注意并发安全（内部使用互斥与条件变量保证）。

---

### 与课程要求的对应关系

- socket：创建五元组对象（本地地址与临时端口初始化），动态分配，返回句柄。
- bind：将服务器 IP/PORT 与 socket 绑定。
- sendto：显式指定目标地址，完成伪首部校验和、交付 IP 层分片、数据链路层发帧。
- recvfrom：网络层接收与分派，校验目的端口和校验和，入队并阻塞读取。
- closesocket：释放五元组资源，返回执行状态。

### 代码定位速查

- 对外导出入口：`net/src/udp/mod.rs`
- 类型与常量：`net/src/udp/types.rs`
- 套接字与队列：`net/src/udp/socket.rs`
- 发送：`net/src/udp/send.rs`
- 接收（入站）：`net/src/udp/recv.rs`
- IPv4 构造/解析：`net/src/ip/builder.rs`、`net/src/ip/parser.rs`
- ARP 与链路层：`net/src/enternet/arp.rs`、`net/src/enternet/datalink/{send.rs,recv.rs}`、`net/src/enternet/frame.rs`

## 核心代码摘录（便于理解实现）

下面将五个公开接口对应的实现核心代码片段摘录自 `net/src/udp/socket.rs`，并作简要说明。可在 VS Code 中打开对应文件查看完整实现。

1) socket / socket_on_iface
- 入口负责参数校验、选择网卡并创建 socket（分配临时端口、注册到全局表）

```rust
// rust
fn choose_iface() -> Result<String> { /* 在 env 或 pcap 中选择设备 */ }

fn create_socket_on_iface(iface: &str) -> Result<SocketId> {
    let local_ip = iface_ipv4(iface)?;
    let local_mac = iface_mac(iface)?;
    let port = next_ephemeral_port();

    let id = next_socket_id();
    let sock = Arc::new(UdpSocket::new(id, iface.to_string(), local_ip, local_mac, port));
    socket_table().write().unwrap().insert(id, Arc::clone(&sock));

    println!("[UDP] 新建 socket id={} 本地IP={} 本地端口={} iface={}", id, crate::enternet::frame::fmt_ipv4(&local_ip), port, iface);
    Ok(id)
}

pub fn socket(af: i32, sock_type: i32, protocol: i32) -> Result<SocketId> {
    if af != AF_INET { return Err(anyhow!("仅支持 AF_INET")); }
    if sock_type != SOCK_DGRAM { return Err(anyhow!("仅支持 SOCK_DGRAM")); }
    if protocol != IPPROTO_IP && protocol != IPPROTO_UDP as i32 { return Err(anyhow!("protocol 仅支持 IPPROTO_IP 或 UDP")); }

    let iface = choose_iface()?;
    create_socket_on_iface(&iface)
}

pub fn socket_on_iface(iface: &str) -> Result<SocketId> {
    create_socket_on_iface(iface)
}
```

2) bind
- 直接修改 socket 内的本地 ip/端口（并打印日志）

```rust
pub fn bind(id: SocketId, addr: SockAddrIn) -> Result<()> {
    let sock = get_socket(id)?;
    sock.bind(addr);
    println!("[UDP] socket id={} 绑定到 {}:{}", id, crate::enternet::frame::fmt_ipv4(&addr.ip), addr.port);
    Ok(())
}
```

3) recvfrom / 阻塞出队（recv_blocking）
- recvfrom 从全局表取 socket 并调用其阻塞接收方法；内部通过 Condvar 唤醒

```rust
pub fn recvfrom(id: SocketId, buf: &mut [u8], _flags: u32) -> Result<(usize, SockAddrIn)> {
    let sock = get_socket(id)?;
    sock.recv_blocking(buf)
}

impl UdpSocket {
    pub(crate) fn recv_blocking(&self, buf: &mut [u8]) -> Result<(usize, SockAddrIn)> {
        let mut inner = self.inner.lock().unwrap();
        loop {
            if let Some(pkt) = inner.queue.pop_front() {
                let n = buf.len().min(pkt.data.len());
                buf[..n].copy_from_slice(&pkt.data[..n]);
                return Ok((n, pkt.src));
            }
            if inner.closed {
                return Err(anyhow!("socket 已关闭"));
            }
            inner = self.recv_cv.wait(inner).unwrap();
        }
    }
}
```

4) closesocket
- 从全局表移除并调用 socket.close() 唤醒阻塞者

```rust
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

impl UdpSocket {
    pub(crate) fn close(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.closed = true;
        inner.queue.clear();
        self.recv_cv.notify_all();
    }
}
```

5) 入队（来自链路层的 handle_udp_packet -> 查表 -> enqueue）
- 全局查表按目的 ip/port 匹配 socket，匹配后通过 enqueue 将包放入 socket 队列并唤醒接收方

```rust
pub(crate) fn find_socket(dst_ip: &[u8; 4], dst_port: u16) -> Option<Arc<UdpSocket>> {
    let table = socket_table().read().unwrap();
    let found = table.values().find(|sock| sock.matches(dst_ip, dst_port)).cloned();
    drop(table);
    found
}

pub(crate) fn enqueue(id: SocketId, packet: Incoming) -> Result<()> {
    let sock = get_socket(id)?;
    sock.enqueue(packet);
    Ok(())
}

impl UdpSocket {
    pub(crate) fn enqueue(&self, packet: Incoming) {
        let mut inner = self.inner.lock().unwrap();
        if inner.closed { return; }
        inner.queue.push_back(packet);
        self.recv_cv.notify_one();
    }

    pub(crate) fn matches(&self, dst_ip: &[u8; 4], dst_port: u16) -> bool {
        let inner = self.inner.lock().unwrap();
        (inner.local_port == dst_port) && (inner.local_ip == *dst_ip || inner.local_ip == [0,0,0,0])
    }
}
```

补充说明
- 全局表：使用 OnceLock<RwLock<HashMap<SocketId, Arc<UdpSocket>>>> 管理 sockets，读多写少的场景较合适。
- 端口分配：EPHEMERAL_PORT 原子增加；若需回收或避免冲突，可改为检查当前表后再分配（当前实现为简单递增）。
- 并发：UdpSocket 内部以 Mutex 保护状态与队列，Condvar 用于阻塞/唤醒 recv；链路层入队时调用 enqueue。

可选优化提示（短）
- 若需要精确端口冲突检测，在分配临时端口时应检查 socket_table 是否已占用该端口（当前仅递增）。
- 队列长度限制与溢出策略（丢弃或替换）目前未实现，生产环境建议添加。
