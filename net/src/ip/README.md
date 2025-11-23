# IPv4 模块简要说明

本模块在网络层实现了一个教学用的简化 IPv4 子系统，按功能拆分为若干子模块并通过 `crate::ip` 暴露接口。

主要功能
- 构造（builder.rs）
  - build_ipv4_packets(payload, params)：按阈值（默认 1400B）和 8B 对齐拆分上层载荷，生成带标识（identification）、DF/MF、偏移、校验和的 IPv4 分片。
- 解析（parser.rs）
  - parse_ipv4_packet(bytes)：验证版本、IHL、总长度与首部校验和，返回解析后的首部、选项与载荷切片。
- 校验和（checksum.rs）
  - ipv4_checksum(header_bytes)：计算 IPv4 首部校验和（16 位反码和）。
- 重组（reassembly.rs）
  - Ipv4Reassembler：按 (src,dst,id,proto) 聚合分片、按偏移排序、检测缺失并在所有分片到齐时返回重组包；提供超时（默认 30s）清理。
- 公共定义（common.rs）
  - Ipv4Header / Ipv4BuildParams / Ipv4Packet / ReassembledPacket 等通用类型与常量。

集成与使用
- 发送端：将上层数据（可附加文件名/长度元数据）交给 `build_ipv4_packets`，再将每个分片当作以太网载荷由数据链路层发送。
- 接收端：数据链路层提取 IPv4 载荷后调用 `parse_ipv4_packet`，分片交 `Ipv4Reassembler` 重组，重组成功后交付上层并写文件（若包含元数据则按原名保存）。
- 单元测试：模块内包含 tests.rs，运行：
  ```
  cd net
  cargo test ipv4
  ```

限制与注意
- 教学用精简实现：选项区固定填充、无完整 ICMP/路由/上层 TCP/UDP 实现。
- 回环（lo0）不可用于以太网层测试，请在真实网卡或虚拟机网络中验证。

```// filepath: /Users/lazy/code/network/net/src/IP/README.md

# IPv4 模块简要说明

本模块在网络层实现了一个教学用的简化 IPv4 子系统，按功能拆分为若干子模块并通过 `crate::ip` 暴露接口。

主要功能
- 构造（builder.rs）
  - build_ipv4_packets(payload, params)：按阈值（默认 1400B）和 8B 对齐拆分上层载荷，生成带标识（identification）、DF/MF、偏移、校验和的 IPv4 分片。
- 解析（parser.rs）
  - parse_ipv4_packet(bytes)：验证版本、IHL、总长度与首部校验和，返回解析后的首部、选项与载荷切片。
- 校验和（checksum.rs）
  - ipv4_checksum(header_bytes)：计算 IPv4 首部校验和（16 位反码和）。
- 重组（reassembly.rs）
  - Ipv4Reassembler：按 (src,dst,id,proto) 聚合分片、按偏移排序、检测缺失并在所有分片到齐时返回重组包；提供超时（默认 30s）清理。
- 公共定义（common.rs）
  - Ipv4Header / Ipv4BuildParams / Ipv4Packet / ReassembledPacket 等通用类型与常量。

集成与使用
- 发送端：将上层数据（可附加文件名/长度元数据）交给 `build_ipv4_packets`，再将每个分片当作以太网载荷由数据链路层发送。
- 接收端：数据链路层提取 IPv4 载荷后调用 `parse_ipv4_packet`，分片交 `Ipv4Reassembler` 重组，重组成功后交付上层并写文件（若包含元数据则按原名保存）。
- 单元测试：模块内包含 tests.rs，运行：
  ```
  cd net
  cargo test ipv4
  ```