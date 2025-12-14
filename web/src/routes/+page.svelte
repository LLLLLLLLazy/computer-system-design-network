<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import QueueItem from '$lib/QueueItem.svelte';

  type QueueInfo = { depth: number; drops: number };
  type Transfer = { role: 'send' | 'recv'; file: string; progress: number; chunks_done: number; chunks_total: number };
  type ArpRow = { ip: string; mac: string; state: string; ttl: string };
  type EventRow = { ts: string; kind: string; text: string };

  type Status = {
    iface: { name: string; ip: string; mac: string };
    counters: { rx_packets: number; tx_packets: number; crc_errors: number; arp_miss: number; udp_recv: number; udp_send: number };
    throughput: { rx_bps: number; tx_bps: number };
    queues: { send: QueueInfo; recv: QueueInfo };
    transfers: Transfer[];
    arp: ArpRow[];
    events: EventRow[];
  };

  const USE_MOCK = false;

  const mockStatus: Status = {
    iface: { name: 'en0', ip: '192.168.1.23', mac: 'b8:27:eb:aa:bb:cc' },
    counters: { rx_packets: 1234, tx_packets: 1180, crc_errors: 2, arp_miss: 5, udp_recv: 640, udp_send: 602 },
    throughput: { rx_bps: 8_200_000, tx_bps: 7_600_000 },
    queues: { send: { depth: 3, drops: 0 }, recv: { depth: 1, drops: 0 } },
    transfers: [
      { role: 'send', file: 'data/input_file.txt', progress: 0.55, chunks_done: 11, chunks_total: 20 },
      { role: 'recv', file: 'received.bin', progress: 0.25, chunks_done: 5, chunks_total: 20 }
    ],
    arp: [
      { ip: '192.168.1.1', mac: 'c8:2a:14:ab:98:ef', state: 'STATIC', ttl: '∞' },
      { ip: '192.168.1.42', mac: 'b8:27:eb:aa:bb:cc', state: 'DYNAMIC', ttl: '580s' }
    ],
    events: [
      { ts: '12:00:01', kind: 'RX', text: '收到 UDP 分片 idx=3' },
      { ts: '12:00:05', kind: 'TX', text: '发送 UDP 分片 idx=4' }
    ]
  };

  let status: Status | null = USE_MOCK ? mockStatus : null;
  let loading = false;
  let error: string | null = null;
  let interval: ReturnType<typeof setInterval> | null = null;

  const fetchStatus = async () => {
    if (USE_MOCK) return;
    loading = true;
    try {
      const res = await fetch('/api/status');
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      status = await res.json();
      error = null;
    } catch (e) {
      error = (e as Error).message;
    } finally {
      loading = false;
    }
  };

  const formatBps = (bps: number) => (bps > 1e6 ? `${(bps / 1e6).toFixed(2)} Mbps` : `${(bps / 1e3).toFixed(1)} kbps`);
  const fmtPct = (p: number) => `${Math.round(p * 100)}%`;

  onMount(() => {
    fetchStatus();
    interval = setInterval(fetchStatus, 3000);
  });

  onDestroy(() => {
    if (interval) clearInterval(interval);
  });
</script>

<svelte:head>
  <title>Network Dashboard</title>
</svelte:head>

<div class="min-h-screen px-6 py-8 space-y-6 max-w-6xl mx-auto">
  <header class="flex items-center justify-between flex-wrap gap-3">
    <div>
      <p class="text-sm text-slate-400">接口</p>
      {#if status}
        <h1 class="text-2xl font-bold">{status.iface.name} · {status.iface.ip} · {status.iface.mac}</h1>
      {:else}
        <h1 class="text-2xl font-bold text-slate-500">等待测试数据...</h1>
      {/if}
    </div>
    <div class="flex gap-2 items-center">
      {#if loading}<span class="text-xs text-slate-400">更新中...</span>{/if}
      {#if error}<span class="text-xs text-red-400">错误: {error}</span>{/if}
      <button class="px-3 py-2 rounded-lg bg-brand-600 hover:bg-brand-700 text-sm font-semibold" on:click={fetchStatus}>
        手动刷新
      </button>
    </div>
  </header>

  {#if status}
    <div class="grid md:grid-cols-3 gap-4">
      <div class="card">
        <p class="text-sm text-slate-400">接收包</p>
        <p class="text-3xl font-bold">{status.counters.rx_packets}</p>
        <p class="text-xs text-slate-400 mt-1">吞吐 {formatBps(status.throughput.rx_bps)}</p>
      </div>
      <div class="card">
        <p class="text-sm text-slate-400">发送包</p>
        <p class="text-3xl font-bold">{status.counters.tx_packets}</p>
        <p class="text-xs text-slate-400 mt-1">吞吐 {formatBps(status.throughput.tx_bps)}</p>
      </div>
      <div class="card">
        <p class="text-sm text-slate-400">错误 / 丢弃</p>
        <p class="text-3xl font-bold">{status.counters.crc_errors}</p>
        <p class="text-xs text-slate-400 mt-1">ARP Miss {status.counters.arp_miss}</p>
      </div>
    </div>

    <div class="grid md:grid-cols-2 gap-4">
      <div class="card">
        <div class="flex items-center justify-between mb-3">
          <p class="font-semibold">队列</p>
          <p class="text-xs text-slate-400">发送 / 接收</p>
        </div>
        <div class="space-y-3">
          <QueueItem title="Send Queue" queue={status.queues.send} />
          <QueueItem title="Recv Queue" queue={status.queues.recv} />
        </div>
      </div>

      <div class="card">
        <div class="flex items-center justify-between mb-3">
          <p class="font-semibold">UDP 传输进度</p>
          <p class="text-xs text-slate-400">分片进度</p>
        </div>
        <div class="space-y-3">
          {#if status.transfers.length === 0}
            <p class="text-sm text-slate-500">未开始 UDP 传输</p>
          {:else}
            {#each status.transfers as t}
              <div class="p-3 rounded-xl bg-slate-900/50 border border-slate-700">
                <div class="flex justify-between text-sm">
                  <span class="font-semibold uppercase text-brand-400">{t.role}</span>
                  <span class="text-slate-400">{fmtPct(t.progress)}</span>
                </div>
                <p class="text-sm mt-1">{t.file}</p>
                <div class="w-full h-2 rounded-full bg-slate-700 mt-2 overflow-hidden">
                  <div class="h-full bg-brand-600" style={`width: ${fmtPct(t.progress)}`}></div>
                </div>
                <p class="text-xs text-slate-400 mt-1">片 {t.chunks_done}/{t.chunks_total}</p>
              </div>
            {/each}
          {/if}
        </div>
      </div>
    </div>

    <div class="grid md:grid-cols-2 gap-4">
      <div class="card">
        <div class="flex items-center justify-between mb-3">
          <p class="font-semibold">ARP 表</p>
          <p class="text-xs text-slate-400">IP / MAC / 状态</p>
        </div>
        <div class="space-y-2 text-sm">
          {#if status.arp.length === 0}
            <p class="text-slate-500">暂无 ARP 数据</p>
          {:else}
            {#each status.arp as a}
              <div class="flex justify-between px-2 py-2 rounded-lg bg-slate-900/50 border border-slate-800">
                <span>{a.ip}</span>
                <span class="text-slate-300">{a.mac}</span>
                <span class="text-xs text-slate-400">{a.state} · {a.ttl}</span>
              </div>
            {/each}
          {/if}
        </div>
      </div>

      <div class="card">
        <div class="flex items-center justify-between mb-3">
          <p class="font-semibold">事件</p>
          <p class="text-xs text-slate-400">最新</p>
        </div>
        <div class="space-y-2 text-sm max-h-64 overflow-auto pr-1">
          {#if status.events.length === 0}
            <p class="text-slate-500">暂无事件</p>
          {:else}
            {#each status.events as e}
              <div class="flex gap-2 px-2 py-2 rounded-lg bg-slate-900/50 border border-slate-800">
                <span class="text-xs text-slate-500">{e.ts}</span>
                <span class="text-xs px-2 py-0.5 rounded-full bg-brand-700/40 border border-brand-700 text-brand-200">{e.kind}</span>
                <span>{e.text}</span>
              </div>
            {/each}
          {/if}
        </div>
      </div>
    </div>
  {:else}
    <div class="card text-center py-12">
      <p class="text-lg font-semibold text-slate-200">尚未开始测试</p>
      <p class="text-sm text-slate-400 mt-2">启动 status_server 并运行一次发送/接收后，这里会显示实时数据。</p>
    </div>
  {/if}

</div>
