# Network Dashboard (SvelteKit + Tailwind)

可视化 enternet / IPv4 / UDP 状态的前端 demo，默认使用 mock 数据；若后端提供 `/api/status`，可切换到实时数据。

## 开发
```bash
cd web
npm install
npm run dev -- --host
# 打开 http://localhost:5173
```

## 构建
```bash
cd web
npm run build
npm run preview
```

## 接口约定 `/api/status`
返回 JSON 示例：
```json
{
  "iface": { "name": "en0", "ip": "192.168.1.23", "mac": "b8:27:eb:aa:bb:cc" },
  "counters": { "rx_packets": 0, "tx_packets": 0, "crc_errors": 0, "arp_miss": 0, "udp_recv": 0, "udp_send": 0 },
  "throughput": { "rx_bps": 0, "tx_bps": 0 },
  "queues": { "send": { "depth": 0, "drops": 0 }, "recv": { "depth": 0, "drops": 0 } },
  "transfers": [{ "role": "send", "file": "data/input_file.txt", "progress": 0.5, "chunks_done": 10, "chunks_total": 20 }],
  "arp": [{ "ip": "192.168.1.1", "mac": "c8:2a:14:ab:98:ef", "state": "STATIC", "ttl": "∞" }],
  "events": [{ "ts": "12:00:01", "kind": "RX", "text": "收到 UDP 分片 idx=3" }]
}
```

在 `src/routes/+page.svelte` 中将 `USE_MOCK` 设为 `false` 即可改用后端数据。建议 Rust 侧用 `axum/warp` 提供该接口，并可加 WebSocket/SSE 推实时事件。
