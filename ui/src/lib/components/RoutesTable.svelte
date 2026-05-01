<!-- src/lib/components/RoutesTable.svelte -->
<script lang="ts">
    import type { RoutePayload } from '$lib/types';
    import type { Theme } from '$lib/theme.svelte';
    import Card from './shared/Card.svelte';
    import SlimBar from './shared/SlimBar.svelte';

    let { routes, t, D }: { routes: RoutePayload[]; t: Theme; D: { fz: number; pad: number; rowPy: number } } = $props();

    function fmtMs(ms: number) {
        if (ms <= 0) return '—';
        return ms >= 1000 ? `${(ms / 1000).toFixed(1)}s` : `${ms.toFixed(0)}ms`;
    }

    let sorted = $derived([...routes].sort((a, b) => b.p95_ms - a.p95_ms));
</script>

<Card {t} title="Routes" sub="{routes.length} endpoints · sorted by p95 desc">
    {#if routes.length === 0}
        <div style="padding:{D.pad}px;color:{t.muted};font-size:{D.fz}px">No route data yet</div>
    {:else}
        <table style="width:100%;border-collapse:collapse;font-family:ui-monospace,monospace;font-size:{D.fz}px">
            <thead>
                <tr>
                    {#each ['Route','Method','RPS','p50','p95','p99','Err %','In-flight','Load'] as h, i}
                        <th style="padding:{D.rowPy + 4}px {D.pad + 2}px {D.rowPy + 2}px;text-align:{i < 2 ? 'left' : 'right'};font-weight:500;font-size:10px;letter-spacing:0.04em;color:{t.muted};text-transform:uppercase;border-bottom:1px solid {t.rule}">{h}</th>
                    {/each}
                </tr>
            </thead>
            <tbody style="font-variant-numeric:tabular-nums">
                {#each sorted as r}
                    {@const p95tone = r.p95_ms > 10000 ? t.err : r.p95_ms > 5000 ? t.warn : t.ink}
                    <tr style="border-bottom:1px solid {t.rule}">
                        <td style="padding:{D.rowPy}px {D.pad + 2}px">{r.path}</td>
                        <td style="padding:{D.rowPy}px {D.pad + 2}px;color:{t.muted}">{r.method}</td>
                        <td style="padding:{D.rowPy}px {D.pad + 2}px;text-align:right">{r.rps.toFixed(1)}</td>
                        <td style="padding:{D.rowPy}px {D.pad + 2}px;text-align:right;color:{t.muted}">{fmtMs(r.p50_ms)}</td>
                        <td style="padding:{D.rowPy}px {D.pad + 2}px;text-align:right;color:{p95tone};font-weight:{r.p95_ms > 5000 ? 600 : 400}">{fmtMs(r.p95_ms)}</td>
                        <td style="padding:{D.rowPy}px {D.pad + 2}px;text-align:right;color:{t.muted}">{fmtMs(r.p99_ms)}</td>
                        <td style="padding:{D.rowPy}px {D.pad + 2}px;text-align:right;color:{r.error_pct > 1 ? t.err : r.error_pct > 0 ? t.warn : t.muted}">{r.error_pct.toFixed(2)}</td>
                        <td style="padding:{D.rowPy}px {D.pad + 2}px;text-align:right">{r.in_flight}</td>
                        <td style="padding:{D.rowPy}px {D.pad + 2}px;width:80px"><SlimBar pct={r.load_pct} {t} h={4} /></td>
                    </tr>
                {/each}
            </tbody>
        </table>
    {/if}
</Card>
