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

## 入站处理流程（`handle_udp_packet`）
1. 在数据链路接收路径中（protocol=17）调用。
2. 校验 UDP 长度与校验和（伪首部 + 首部 + 数据）。
3. 依据目的 IP/端口在全局表中查找匹配 socket；若无匹配仅打印提示（未实现 ICMP 端口不可达）。
4. 将去掉首部后的数据及源地址封装为 `Incoming` 入队；`recvfrom` 会阻塞等待。

## 文件结构
- `mod.rs`：对外导出接口。
- `types.rs`：常量与地址类型。
- `socket.rs`：socket 生命周期、全局表、队列管理。
- `send.rs`：`sendto` 实现、UDP 报文与校验和、IPv4/ARP/pcap 发送。
- `recv.rs`：`handle_udp_packet` 入站校验与投递。

## 使用提示
- 依赖 pcap，运行前请确保有权限抓发包。
- 通过环境变量 `NET_IFACE` 可显式指定网卡；否则取第一个 pcap 设备。
- 目前未发送 ICMP 端口不可达；需要可在 `recv.rs` 查找失败分支补充。
