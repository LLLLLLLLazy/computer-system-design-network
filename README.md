# computer-system-design-network

面向《计算机系统设计》课程的网络实验项目集合，当前已经实现 Rust 版以太网帧收发工具 **enternet**，涵盖数据链路层帧构造、抓包与多线程队列化处理。

## 目录结构
```
network/
├── README.md                # 项目说明（本文件）
└── net/                     # enternet 子项目
    ├── Cargo.toml
    ├── data/                # 输入/输出数据
    │   ├── input_file.txt
    │   └── output_file.txt
    └── src/
        ├── main.rs          # 程序入口
        └── enternet/        # 核心模块
            ├── cli.rs       # 命令行解析
            ├── datalink/    # 发送与接收实现
            ├── frame.rs     # 帧格式常量与工具
            ├── net.rs       # MAC 查询与解析
            ├── recv_queue.rs# 接收队列（消费者）
            └── send_queue.rs# 发送队列（生产者）
```

## 已完成功能

### 数据链路层收发
- **帧构造**：从 `data/input_file.txt` 读取数据，按以太网规范填充源/目的 MAC、EtherType、载荷与 CRC32，自动补齐最小帧长。
- **真实 MAC 支持**：自动获取指定网卡的本机 MAC，发送时接受目标 MAC 参数。
- **发送队列**：`SendQueue` 将构帧与 `pcap_sendpacket` 分离，避免 I/O 阻塞并支持后续扩展。
- **接收队列**：`RecvQueue` 缓存校验通过的帧，由独立线程交付上层（当前覆盖写入 `data/output_file.txt`），便于多线程/多进程处理。
- **CRC 校验**：`frame::crc32` 对发送帧附加 32 位 CRC；接收端验证 CRC 并过滤非法帧。
- **过滤与日志**：安装 BPF 过滤器（IPv4），打印源/目的 MAC、帧长度、CRC 等信息。

### 多线程架构
- 发送：主线程读取文件 → 入队，工作线程出队并发包。
- 接收：抓包线程校验 → 入队，交付线程出队写文件。
- 队列生命周期：初始化 → 入队 → 出队 → 关闭，体现多进程/多线程流水线思想。

## 构建与运行

进入 `net` 子项目目录：

- 编译（可选）：

  ```bash
  make clean
  make build
  ```

- 发送（若不提供 iface 或 目标mac，程序会交互提示选择/输入）：

  ```bash
  make run-send <iface> <dest-mac>
  ```

- 接收（若不提供 iface，程序会交互提示选择/输入）：

  ```bash
  make run-recv <iface>
  ```


如果想要直接运行可执行程序：
```bash
#net\target\debug
#进入debug目录下运行
./net recv          # 接收模式
./net send          # 发送模式
```
- `<iface>`：本机网卡名称（如 `en0`、`eth0`）。
- `<dest-mac>`：目标主机 MAC 地址（`XX:XX:XX:XX:XX:XX`）。
- 仅可在拥有 libpcap 权限的环境运行。