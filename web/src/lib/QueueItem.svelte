<script lang="ts">
  export let queue: { depth: number; drops: number };
  export let title: string;
</script>

<div class="flex items-center justify-between p-3 rounded-xl bg-slate-900/50 border border-slate-700">
  <div>
    <p class="text-sm font-semibold">{title}</p>
    <p class="text-xs text-slate-400">Depth {queue.depth} · Drops {queue.drops}</p>
  </div>
  <div class="text-xs px-3 py-1 rounded-full bg-brand-700/30 border border-brand-700">
    {queue.depth <= 3 ? 'OK' : 'Busy'}
  </div>
</div>
