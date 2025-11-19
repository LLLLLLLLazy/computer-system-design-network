# Enternet

轻量级以太网帧收发示例，演示基于 libpcap 的数据链路层开发、CRC 校验及多线程队列化处理流程。

## 功能概览
- 发送：读取 `data/input_file.txt`，构造以太网帧并通过指定网卡发送，发送路径采用生产者-消费者队列解耦构帧与发包。
- 接收：捕获网卡上的 IPv4 帧，验证目的 MAC、帧长与 CRC，交付线程将完整帧覆盖写入 `data/output_file.txt`。
- MAC：自动获取本机源 MAC，发送时支持 CLI 传入目标 MAC。
- 队列：`SendQueue` 与 `RecvQueue` 支持初始化、入队、出队与关闭，体现多进程/多线程流水线思想。

## 目录结构
```
src/
  enternet/
    cli.rs          // 命令行解析
    datalink/       // 链路层收发实现
    frame.rs        // 帧常量与工具
    net.rs          // MAC 查询与解析
    recv_queue.rs   // 接收队列
    send_queue.rs   // 发送队列
```

## 构建与运行
```bash
cargo build
sudo cargo run -- send <iface> <dest-mac>
sudo cargo run -- recv <iface>
```
- `<iface>`：实际网卡名称，如 `en0`。
- `<dest-mac>`：目标主机 MAC，格式 `11:22:33:44:55:66`。

## 形成 enternet 帧的接口使用方法

### 1. 高层入口
```rust
datalink_send(iface: &str, src_mac: [u8; 6], dest_mac: [u8; 6]) -> Result<()>
datalink_recv(iface: &str, local_mac: [u8; 6]) -> Result<()>
```
发送流程：
1. 高层读取文件数据，按 `MAX_PAYLOAD_LEN` 切片；
2. 每个切片交给 `build_frame` 生成完整以太网帧；
3. 帧被入队到 `SendQueue`，发送线程取出并调用 `pcap::Capture::sendpacket` 发出；
4. 队列关闭唤醒线程完成收尾。

接收流程：
1. `datalink_recv` 启动抓包线程与交付线程，并创建 `RecvQueue`；
2. 抓包线程循环调用 `pcap::Capture::next_packet`；
3. `handle_frame` 校验目的 MAC、长度与 CRC，通过后返回完整帧；
4. 帧入队 `RecvQueue`，队列满时记录并丢弃；
5. 交付线程从队列取帧，经 `deliver_frame` 覆盖写入 `data/output_file.txt`；
6. 关闭队列唤醒等待线程并安全退出。

### 2. 构帧细节
- 头部：`dest_mac || src_mac || ether_type (0x0800)`；
- 载荷：不足 46 字节自动补零；
- CRC：`frame`（去掉 CRC 部分）通过 `frame::crc32` 计算并附加；
- 帧长：不足 64 字节时整体补零对齐。

相关函数：
- `frame::crc32(data: &[u8]) -> u32`
- `frame::MIN_PAYLOAD_LEN`, `MIN_FRAME_SIZE` 等常量定义帧约束。