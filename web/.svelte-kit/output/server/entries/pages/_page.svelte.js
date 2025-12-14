import { y as bind_props, w as head, z as ensure_array_like, F as attr_style } from "../../chunks/index.js";
import { a as ssr_context, e as escape_html } from "../../chunks/context.js";
import "clsx";
function onDestroy(fn) {
  /** @type {SSRContext} */
  ssr_context.r.on_destroy(fn);
}
function QueueItem($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let queue = $$props["queue"];
    let title = $$props["title"];
    $$renderer2.push(`<div class="flex items-center justify-between p-3 rounded-xl bg-slate-900/50 border border-slate-700"><div><p class="text-sm font-semibold">${escape_html(title)}</p> <p class="text-xs text-slate-400">Depth ${escape_html(queue.depth)} · Drops ${escape_html(queue.drops)}</p></div> <div class="text-xs px-3 py-1 rounded-full bg-brand-700/30 border border-brand-700">${escape_html(queue.depth <= 3 ? "OK" : "Busy")}</div></div>`);
    bind_props($$props, { queue, title });
  });
}
function _page($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    const mockStatus = {
      iface: { name: "en0", ip: "192.168.1.23", mac: "b8:27:eb:aa:bb:cc" },
      counters: {
        rx_packets: 1234,
        tx_packets: 1180,
        crc_errors: 2,
        arp_miss: 5
      },
      throughput: { rx_bps: 82e5, tx_bps: 76e5 },
      queues: { send: { depth: 3, drops: 0 }, recv: { depth: 1, drops: 0 } },
      transfers: [
        {
          role: "send",
          file: "data/input_file.txt",
          progress: 0.55,
          chunks_done: 11,
          chunks_total: 20
        },
        {
          role: "recv",
          file: "received.bin",
          progress: 0.25,
          chunks_done: 5,
          chunks_total: 20
        }
      ],
      arp: [
        {
          ip: "192.168.1.1",
          mac: "c8:2a:14:ab:98:ef",
          state: "STATIC",
          ttl: "∞"
        },
        {
          ip: "192.168.1.42",
          mac: "b8:27:eb:aa:bb:cc",
          state: "DYNAMIC",
          ttl: "580s"
        }
      ],
      events: [
        { ts: "12:00:01", kind: "RX", text: "收到 UDP 分片 idx=3" },
        { ts: "12:00:05", kind: "TX", text: "发送 UDP 分片 idx=4" }
      ]
    };
    let status = mockStatus;
    const formatBps = (bps) => bps > 1e6 ? `${(bps / 1e6).toFixed(2)} Mbps` : `${(bps / 1e3).toFixed(1)} kbps`;
    const fmtPct = (p) => `${Math.round(p * 100)}%`;
    onDestroy(() => {
    });
    head("1uha8ag", $$renderer2, ($$renderer3) => {
      $$renderer3.title(($$renderer4) => {
        $$renderer4.push(`<title>Network Dashboard</title>`);
      });
    });
    $$renderer2.push(`<div class="min-h-screen px-6 py-8 space-y-6 max-w-6xl mx-auto"><header class="flex items-center justify-between flex-wrap gap-3"><div><p class="text-sm text-slate-400">接口</p> <h1 class="text-2xl font-bold">${escape_html(status.iface.name)} · ${escape_html(status.iface.ip)} · ${escape_html(status.iface.mac)}</h1></div> <div class="flex gap-2 items-center">`);
    {
      $$renderer2.push("<!--[!-->");
    }
    $$renderer2.push(`<!--]--> `);
    {
      $$renderer2.push("<!--[!-->");
    }
    $$renderer2.push(`<!--]--> <button class="px-3 py-2 rounded-lg bg-brand-600 hover:bg-brand-700 text-sm font-semibold">手动刷新</button></div></header> <div class="grid md:grid-cols-3 gap-4"><div class="card"><p class="text-sm text-slate-400">接收包</p> <p class="text-3xl font-bold">${escape_html(status.counters.rx_packets)}</p> <p class="text-xs text-slate-400 mt-1">吞吐 ${escape_html(formatBps(status.throughput.rx_bps))}</p></div> <div class="card"><p class="text-sm text-slate-400">发送包</p> <p class="text-3xl font-bold">${escape_html(status.counters.tx_packets)}</p> <p class="text-xs text-slate-400 mt-1">吞吐 ${escape_html(formatBps(status.throughput.tx_bps))}</p></div> <div class="card"><p class="text-sm text-slate-400">错误 / 丢弃</p> <p class="text-3xl font-bold">${escape_html(status.counters.crc_errors)}</p> <p class="text-xs text-slate-400 mt-1">ARP Miss ${escape_html(status.counters.arp_miss)}</p></div></div> <div class="grid md:grid-cols-2 gap-4"><div class="card"><div class="flex items-center justify-between mb-3"><p class="font-semibold">队列</p> <p class="text-xs text-slate-400">发送 / 接收</p></div> <div class="space-y-3">`);
    QueueItem($$renderer2, { title: "Send Queue", queue: status.queues.send });
    $$renderer2.push(`<!----> `);
    QueueItem($$renderer2, { title: "Recv Queue", queue: status.queues.recv });
    $$renderer2.push(`<!----></div></div> <div class="card"><div class="flex items-center justify-between mb-3"><p class="font-semibold">UDP 传输进度</p> <p class="text-xs text-slate-400">分片进度</p></div> <div class="space-y-3"><!--[-->`);
    const each_array = ensure_array_like(status.transfers);
    for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
      let t = each_array[$$index];
      $$renderer2.push(`<div class="p-3 rounded-xl bg-slate-900/50 border border-slate-700"><div class="flex justify-between text-sm"><span class="font-semibold uppercase text-brand-400">${escape_html(t.role)}</span> <span class="text-slate-400">${escape_html(fmtPct(t.progress))}</span></div> <p class="text-sm mt-1">${escape_html(t.file)}</p> <div class="w-full h-2 rounded-full bg-slate-700 mt-2 overflow-hidden"><div class="h-full bg-brand-600"${attr_style(`width: ${fmtPct(t.progress)}`)}></div></div> <p class="text-xs text-slate-400 mt-1">片 ${escape_html(t.chunks_done)}/${escape_html(t.chunks_total)}</p></div>`);
    }
    $$renderer2.push(`<!--]--></div></div></div> <div class="grid md:grid-cols-2 gap-4"><div class="card"><div class="flex items-center justify-between mb-3"><p class="font-semibold">ARP 表</p> <p class="text-xs text-slate-400">IP / MAC / 状态</p></div> <div class="space-y-2 text-sm"><!--[-->`);
    const each_array_1 = ensure_array_like(status.arp);
    for (let $$index_1 = 0, $$length = each_array_1.length; $$index_1 < $$length; $$index_1++) {
      let a = each_array_1[$$index_1];
      $$renderer2.push(`<div class="flex justify-between px-2 py-2 rounded-lg bg-slate-900/50 border border-slate-800"><span>${escape_html(a.ip)}</span> <span class="text-slate-300">${escape_html(a.mac)}</span> <span class="text-xs text-slate-400">${escape_html(a.state)} · ${escape_html(a.ttl)}</span></div>`);
    }
    $$renderer2.push(`<!--]--></div></div> <div class="card"><div class="flex items-center justify-between mb-3"><p class="font-semibold">事件</p> <p class="text-xs text-slate-400">最新</p></div> <div class="space-y-2 text-sm max-h-64 overflow-auto pr-1"><!--[-->`);
    const each_array_2 = ensure_array_like(status.events);
    for (let $$index_2 = 0, $$length = each_array_2.length; $$index_2 < $$length; $$index_2++) {
      let e = each_array_2[$$index_2];
      $$renderer2.push(`<div class="flex gap-2 px-2 py-2 rounded-lg bg-slate-900/50 border border-slate-800"><span class="text-xs text-slate-500">${escape_html(e.ts)}</span> <span class="text-xs px-2 py-0.5 rounded-full bg-brand-700/40 border border-brand-700 text-brand-200">${escape_html(e.kind)}</span> <span>${escape_html(e.text)}</span></div>`);
    }
    $$renderer2.push(`<!--]--></div></div></div></div>`);
  });
}
export {
  _page as default
};
