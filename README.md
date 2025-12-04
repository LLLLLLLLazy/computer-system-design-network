# computer-system-design-network

面向《计算机系统设计》课程的网络实验项目集合。当前实现 Rust 版以太网帧收发工具 **enternet**，并已实现 IPv4（网络层）与 ARP（地址解析协议）功能，支持分片/重组、首部校验和与自动 MAC 解析。

## 目录结构
```
network/
└── net/
    ├── Cargo.toml
    ├── Makefile
    ├── data/
    │   ├── input_file.txt
    │   ├── output_file.txt
    │   └── tools/gen_and_verify.py
    └── src/
        ├── main.rs
        ├── config.rs        # 常量与网络参数（子网掩码/网关/DNS/DHCP 等）
        ├── cli/             # 命令行解析与交互
        ├── enternet/        # 数据链路层（帧、队列、ARP、datalink）
        │   ├── frame.rs     # 以太网帧常量与工具（MAC/ETHertype/CRC）
        │   ├── arp.rs       # ARP 报文与缓存、解析与应答
        │   ├── net.rs
        │   └── datalink/
        │       ├── send.rs  # 发送路径（ARP 解析后发 IPv4）
        │       └── recv.rs  # 接收路径（监听 ARP/IPv4）
        └── ip/              # 网络层 IPv4 模块（构造/解析/重组/校验和）
            ├── common.rs
            ├── builder.rs
            ├── parser.rs
            ├── reassembly.rs
            ├── checksum.rs
            └── tests.rs
```

## 已完成功能
- 数据链路层（enternet）
  - 以太网帧构造（`src/enternet/frame.rs`）：
    - 以太网头（目的/源 MAC + EtherType）。
    - 载荷：IPv4 报文或任意字节序列。
    - CRC32 计算与附加；接收端验证 CRC，错误帧丢弃并记录日志。
    - 最小帧长度补齐到 64 字节。
  - 发送流程（`datalink/send.rs`）：
    - 读取 `data/input_file.txt`，支持应用元数据（文件名与长度）。
    - 将数据拆成 IPv4 分片（见 IP 模块），为每片构建以太网帧并入队，由发送线程通过 libpcap 发送。
  - 接收流程（`datalink/recv.rs`）：
    - 通过 libpcap 捕获帧（BPF：`arp or ether proto 0x0800`）。
    - 验证长度与 CRC，剥离载荷交付上层 IPv4。
    - 打印源/目的 MAC、EtherType、帧长等信息。
  - 队列：SendQueue / RecvQueue 解耦抓包/发送与磁盘 I/O。

- 网络层 IPv4（`src/ip/`）
  - 分片构造（`builder.rs`）：按 1400B 阈值、8 字节对齐生成分片，设置标志位与片偏移，计算首部校验和。
  - 首部解析（`parser.rs`）：校验版本、IHL、总长度、校验和、标志位与片偏移。
  - 分片重组（`reassembly.rs`）：缓存分片按偏移重组，支持超时清理。
  - 校验和（`checksum.rs`）：IPv4 首部校验和计算。
  - 单元测试（`tests.rs`）：覆盖分片/重组与校验和等核心逻辑。

## 本次增量（ARP 协议）
- 新增 `src/enternet/arp.rs`：
  - ARP 报文字段定义与帧构造（请求/应答）。
  - ARP 缓存（静态/动态/TTL=10min），3 次重试 + 10s 超时策略。
  - 根据子网掩码判断同网段/跨网段，自动解析目标 IP 或网关 IP 的 MAC。
- 发送路径集成（`datalink/send.rs`）：
  - 在发送 IPv4 前先解析目的 MAC（命令行可选指定静态 MAC；否则触发 ARP）。
- 接收路径集成（`datalink/recv.rs`）：
  - 监听 ARP 与 IPv4，收到针对本机的 ARP 请求时即时回复。
  - 将网络上的 ARP 应答学习写入应用缓存，避免发送端超时。
- 配置（`config.rs`）：支持环境变量覆盖网络参数（`NET_SUBNET_MASK`、`NET_GATEWAY_IP`、`NET_DNS1`、`NET_DNS2`、`NET_DHCP`）。
- CLI：`send <iface> <dest-ip> [protocol] [dest-mac]`，`dest-mac` 为空时自动 ARP 解析。

## 运行与测试
1. 构建
   ```bash
   cd net
   make build   # 或 cargo build
   ```

2. 生成测试数据
   ```bash
   python3 net/data/tools/gen_and_verify.py generate 4096
   ```

3. 单元测试（IPv4 模块）
   ```bash
   cd net
   cargo test
   cargo test ipv4
   ```

4. 端到端测试（含 ARP）
   - 接收端（机器 A / 终端 1）：
     ```bash
     cd net
     sudo cargo run -- recv <iface>
     # 或
     make run-recv
     ```
   - 发送端（机器 B / 终端 2）：
     ```bash
     cd net
     sudo cargo run -- send <iface> <dest-ip> [protocol] [dest-mac]
     # 或
     make run-send
     ```
     提示：
     - `dest-mac` 可选；留空会广播 ARP 请求并自动学习应答。
     - 若网络设备开启客户端隔离（Wi‑Fi AP Isolation），ARP 可能无法到达对端，建议在同一二层网络或改用有线。
     - 如需覆盖网络参数，设置环境变量后再运行命令：`NET_SUBNET_MASK=255.255.255.0 NET_GATEWAY_IP=192.168.1.1 sudo cargo run ...`
   - 验证传输完成：
     ```bash
     python3 net/data/tools/gen_and_verify.py test
     ```

5. 可执行文件直接运行
   ```bash
   cd net/target/debug
   sudo ./net recv
   sudo ./net send
   ```

## 注意事项与限制
- 教学实现：未实现 ICMP 回显、未解析 TCP/UDP 载荷，仅保留协议号与原始数据。
- 需在非生产网络中测试，避免影响实际业务。
- 发送/抓包需 libpcap 权限（通常 sudo/root）。
- 回环接口（lo0）无以太网帧与 MAC 层，不适用于 ARP/以太网层测试。
