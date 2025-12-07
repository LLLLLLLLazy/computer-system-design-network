# UDP 测试说明

## 测试层次
- `simulate_udp_delivery_like_real_world`: 纯内存模拟，构造带伪首部校验和的 UDP 段与 IPv4 头，经过 `handle_udp_packet` -> `recvfrom` 完整接收路径，无需 pcap/网卡。
- `inject_udp_payload_to_socket_queue` (`#[ignore]`): 依赖本机 NET_IFACE 与 pcap，手动构造报文注入，默认跳过。
- `api::*`: 针对接口的粒度测试（`bind`/`recvfrom`/`udp_checksum`/`closesocket` 等），在内存注册测试 socket，不依赖实际网卡。

## 运行方式
- 全量运行并打印日志：
  ```bash
  cargo test -- --nocapture
  ```
- 仅运行某个测试：
  ```bash
  cargo test simulate_udp_delivery_like_real_world -- --nocapture
  cargo test udp::test::api::api_recvfrom_after_handle_udp -- --nocapture
  ```
- 包含 ignored 测试（可能需要 NET_IFACE 与 pcap 权限）：
  ```bash
  cargo test -- --ignored --nocapture
  cargo test inject_udp_payload_to_socket_queue -- --ignored --nocapture
  ```

## 环境说明
- 非 pcap 测试无需真实网卡，使用 `register_test_socket` 注册内存 socket。
- `inject_udp_payload_to_socket_queue` 依赖：
  - 环境变量 `NET_IFACE` 指向可用网卡。
  - 运行用户需有 pcap 抓发包权限（macOS 可通过 `sudo chmod` 或安装时授予权限）。

## 日志
- 测试中有 `println!` 输出，使用 `--nocapture` 可在终端直接看到收发细节与校验和结果。
