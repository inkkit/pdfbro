<!-- src/lib/components/Ticker.svelte -->
<script lang="ts">
    import type { TickerPayload } from '$lib/types';
    import type { Theme } from '$lib/theme.svelte';

    let { ticker, t, D }: { ticker: TickerPayload; t: Theme; D: { kpiFz: number; fz: number; pad: number } } = $props();

    function fmtMs(ms: number) {
        return ms >= 1000 ? `${(ms / 1000).toFixed(1)}s` : `${ms.toFixed(0)}ms`;
    }

    function fmtUptime(s: number) {
        const h = Math.floor(s / 3600);
        const m = Math.floor((s % 3600) / 60);
        return `${h}h ${m}m`;
    }

    let items = $derived([
        { label: 'RPS',    value: ticker.rps.toFixed(1),                      tone: 'ink' as const },
        { label: 'P50',    value: fmtMs(ticker.p50_ms),                       tone: (ticker.p50_ms > 1000 ? 'err' : ticker.p50_ms > 500 ? 'warn' : 'ok') as 'ok' | 'warn' | 'err' },
        { label: 'P55',    value: fmtMs(ticker.p55_ms),                       tone: (ticker.p55_ms > 1000 ? 'err' : ticker.p55_ms > 500 ? 'warn' : 'ok') as 'ok' | 'warn' | 'err' },
        { label: 'P95',    value: fmtMs(ticker.p95_ms),                       tone: (ticker.p95_ms > 2000 ? 'err' : ticker.p95_ms > 1500 ? 'warn' : 'ok') as 'ok' | 'warn' | 'err' },
        { label: '5XX',    value: `${ticker.server_error_pct.toFixed(2)}%`,   tone: (ticker.server_error_pct > 1 ? 'err' : ticker.server_error_pct > 0 ? 'warn' : 'ok') as 'ok' | 'warn' | 'err' },
        { label: '429',    value: `${ticker.rate_limit_pct.toFixed(2)}%`,     tone: (ticker.rate_limit_pct > 5 ? 'err' : ticker.rate_limit_pct > 0 ? 'warn' : 'ok') as 'ok' | 'warn' | 'err' },
        { label: 'Conc.',  value: `${ticker.concurrency_active} / ${ticker.concurrency_max}`, tone: 'ink' as const },
        { label: 'Queue',  value: String(Math.round(ticker.queue_size)),       tone: 'ink' as const },
        { label: 'Uptime', value: fmtUptime(ticker.uptime_seconds),            tone: 'ok' as const },
    ]);
</script>

<div style="background:{t.surface};border:1px solid {t.rule};border-radius:12px;display:grid;grid-template-columns:repeat({items.length},1fr)">
    {#each items as item, i}
        {@const color = t[item.tone as keyof Theme] as string ?? t.ink}
        <div style="padding:{D.pad}px {D.pad + 2}px;{i < items.length - 1 ? `border-right:1px solid ${t.rule}` : ''}">
            <div style="color:{t.muted};font-size:10px;letter-spacing:0.06em;text-transform:uppercase;font-weight:500">{item.label}</div>
            <div style="font-family:ui-monospace,monospace;font-size:{D.kpiFz}px;font-weight:600;margin-top:2px;letter-spacing:-0.01em;color:{color}">{item.value}</div>
        </div>
    {/each}
</div>
