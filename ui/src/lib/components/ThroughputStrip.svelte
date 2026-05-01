<!-- ui/src/lib/components/ThroughputStrip.svelte -->
<script lang="ts">
    import type { ThroughputPayload } from '$lib/types';
    import type { Theme } from '$lib/theme.svelte';
    import Card from './shared/Card.svelte';

    let { throughput, t, D }: { throughput: ThroughputPayload; t: Theme; D: { pad: number; fz: number } } = $props();

    let lastRps = $derived(throughput.rps_series.at(-1) ?? 0);
    let lastP95 = $derived(throughput.p95_series.at(-1) ?? 0);
    let maxRps  = $derived(Math.max(...throughput.rps_series, throughput.rps_baseline, 0.01));
    let maxP95  = $derived(Math.max(...throughput.p95_series, throughput.p95_target_s, 0.01));

    function barColor(v: number, max: number, target: number): string {
        const ratio = v / max;
        const overTarget = target > 0 && v > target * 1.2;
        if (overTarget) return t.err;
        if (ratio > 0.8) return t.warn;
        return t.ok;
    }
</script>

<div style="display:grid;grid-template-columns:1fr 1fr;gap:{D.pad}px">
    <Card {t} title="Requests / sec" sub="last 30 min{throughput.rps_baseline > 0 ? ` · baseline ${throughput.rps_baseline.toFixed(0)}` : ''}">
        <div style="padding:{D.pad + 2}px">
            <div style="display:flex;justify-content:space-between;font-size:{D.fz - 1}px;color:{t.muted};margin-bottom:6px">
                <span>RPS</span>
                <span style="font-family:ui-monospace,monospace;color:{t.ink};font-weight:600">{lastRps.toFixed(1)}</span>
            </div>
            <div style="display:flex;align-items:flex-end;gap:1px;height:64px;position:relative">
                {#if throughput.rps_baseline > 0}
                    <div style="position:absolute;left:0;right:0;bottom:{Math.round((throughput.rps_baseline / maxRps) * 64)}px;height:1px;background:{t.warn};opacity:0.5;pointer-events:none"></div>
                {/if}
                {#each throughput.rps_series as v}
                    <div style="flex:1;min-width:2px;background:{barColor(v, maxRps, 0)};height:{Math.max(2, Math.round((v / maxRps) * 64))}px;border-radius:1px 1px 0 0"></div>
                {/each}
            </div>
        </div>
    </Card>

    <Card {t} title="Latency p95" sub="seconds · target < {throughput.p95_target_s}s">
        <div style="padding:{D.pad + 2}px">
            <div style="display:flex;justify-content:space-between;font-size:{D.fz - 1}px;color:{t.muted};margin-bottom:6px">
                <span>p95</span>
                <span style="font-family:ui-monospace,monospace;color:{t.ink};font-weight:600">{lastP95.toFixed(2)}<span style="color:{t.muted};font-weight:400">s</span></span>
            </div>
            <div style="display:flex;align-items:flex-end;gap:1px;height:64px;position:relative">
                <div style="position:absolute;left:0;right:0;bottom:{Math.round((throughput.p95_target_s / maxP95) * 64)}px;height:1px;background:{t.err};opacity:0.5;pointer-events:none"></div>
                {#each throughput.p95_series as v}
                    <div style="flex:1;min-width:2px;background:{barColor(v, maxP95, throughput.p95_target_s)};height:{Math.max(2, Math.round((v / maxP95) * 64))}px;border-radius:1px 1px 0 0"></div>
                {/each}
            </div>
        </div>
    </Card>
</div>
