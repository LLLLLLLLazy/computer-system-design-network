# computer-system-design-network

面向《计算机系统设计》课程的网络实验项目集合。当前实现 Rust 版以太网帧收发工具 **enternet**，并已增量加入简化的 IPv4（网络层）实现，支持分片/重组与首部校验和。

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
        ├── config.rs        # 可选：集中常量（INPUT/OUTPUT、队列容量等）
        ├── cli/             # 命令行解析与交互
        ├── enternet/        # 数据链路层（帧、队列、datalink）
        │   ├── frame.rs     # 以太网帧常量与工具（CRC、格式、MAC/ETHertype）
        │   ├── net.rs
        │   └── datalink/
        │       ├── send.rs
        │       └── recv.rs
        └── ip/              # 新增：网络层 IPv4 模块（构造/解析/重组）
            ├── common.rs
            ├── builder.rs
            ├── parser.rs
            ├── reassembly.rs
            ├── checksum.rs
            └── tests.rs
```

## 已完成功能（数据链路层 enternet）
- 以太网帧构造（位于 `src/enternet/frame.rs`）：
  - 以太网头（14 字节）：目的 MAC (6B) | 源 MAC (6B) | EtherType (2B)。
  - 载荷：IPv4 报文（由 `ip::builder` 生成的字节流）或任意上层字节序列。
  - CRC32（4B）：发送端在帧尾计算并附加 CRC32，接收端验证 CRC，CRC 错误的帧将被丢弃并打印日志。
  - 最小帧长度补齐：若整个帧（含 CRC）小于 64 字节，则在尾部填 0 以满足最小帧长。
  - 常量与工具：EtherType 常量（0x0800）、广播 MAC、CRC 函数、frame 大小常量均集中在 `frame.rs`。
- 发送流程（`datalink/send.rs`）：
  - 读取 `data/input_file.txt`（任意二进制），可先在应用层拼接元数据（文件名长度+文件名+原始长度）。
  - 调用 `ip::build_ipv4_packets` 得到一个或多个 IPv4 分片字节数组。
  - 为每个 IPv4 分片构建以太网帧（填充目的/源 MAC、EtherType、载荷、CRC、最小长度补齐），将帧入发送队列，由发送线程通过 libpcap 发送。
  - 发送过程中打印分片信息（标识/片偏移/DF/MF/总长/载荷长度）与帧发送结果（接口/长度/CRC）。
- 接收流程（`datalink/recv.rs`）：
  - 通过 libpcap 捕获帧（支持 BPF 过滤 EtherType=0x0800）。
  - 校验 CAP 长度与 CRC，若通过则去掉 CRC 并把以太网载荷交给上层（IPv4 解析）。
  - 打印接收到的帧信息（源/目的 MAC、EtherType、长度）。
- 多线程队列：SendQueue / RecvQueue 将捕包/解析与磁盘 I/O 解耦，避免阻塞捕包线程。

## 本次增量（IPv4 模块）要点
- 新增 `src/ip/`，按功能拆分为：头部/构造/解析/重组/校验和，提供：
  - build_ipv4_packets：按 1400B 阈值（并按 8 字节对齐）生成分片，设置 flags/offset/identification、计算首部校验和。
  - parse_ipv4_packet：解析并校验 IPv4 首部（版本、IHL、总长度、校验和、标志位、片偏移等）。
  - Ipv4Reassembler：缓存分片并按偏移排序重组，支持 30s 超时清理。
  - 可选应用元数据：在 IP 载荷前加文件名/长度信息，接收端可按原名写入文件。
- 发送端：将 IP 分片作为以太网载荷发送。
- 接收端：验证目的 IP（本机或广播）、检验和，若为分片则交给重组模块，重组完成后交付并写文件。

## 运行与测试
1. 构建
   ```bash
   cd net
   make build   # 或 cargo build
   ```

2. 生成测试数据
   ```bash
   python3 net/data/tools/gen_and_verify.py generate 4096
   # 或交互： python3 net/data/tools/gen_and_verify.py （脚本支持交互模式）
   ```

3. 单元测试（IPv4 模块）
   ```bash
   cd net
   cargo test
   cargo test ipv4
   ```

4. 端到端测试（分片/重组）
   - 接收端（机器 A / 终端 1）：
     ```bash
     cd net
     sudo cargo run -- recv <iface>
     ```
     或使用 Makefile交互：
     ```bash
     make run-recv
     ```
   - 发送端（机器 B / 终端 2）：
     ```bash
     cd net
     sudo cargo run -- send <iface> <dest-mac> <dest-ip> <protocol>
     ```
     或使用 Makefile交互：
     ```bash
     make run-send
     ```
   - 如果已有编译后的可执行文件，想要直接调用可以使用下面的命令进行交互
      ```bash
     cd net/target/debug
     sudo ./net recv  #接收
     sudo ./net send  #发送
     ```
   - 传输完成后校验：
     ```bash
     python3 net/data/tools/gen_and_verify.py test
     ```
     校验脚本比对 `net/data/input_file.txt` 与 `net/data/output_file.txt`（或重组写出的原名文件）的 SHA256。

## 注意事项与限制
- 实验/教学实现：IPv4 选项区固定填充（40B 全 0），无 ICMP 回复实现（仅打印），未解析 TCP/UDP 上层协议（仅保留协议号）。
- 测试前请在隔离网络或虚拟机中运行，避免影响真实网络环境。
- 发送/捕包需 libpcap 权限（通常 sudo/root）。
- 若使用回环接口（lo0）请注意：loopback 没有 MAC 层帧，不适用于以太网层测试。
