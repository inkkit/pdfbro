<!-- ui/src/lib/components/ThroughputStrip.svelte -->
<script lang="ts">
    import type { ThroughputPayload } from '$lib/types';
    import type { Theme } from '$lib/theme.svelte';
    import Card from './shared/Card.svelte';
    import BarSeries from './shared/BarSeries.svelte';

    let { throughput, t, D }: { throughput: ThroughputPayload; t: Theme; D: { pad: number; fz: number } } = $props();

    let lastRps = $derived(throughput.rps_series.at(-1) ?? 0);
    let lastP95 = $derived(throughput.p95_series.at(-1) ?? 0);
    let maxRps  = $derived(Math.max(...throughput.rps_series, throughput.rps_baseline, 0.001));
    let maxP95  = $derived(Math.max(...throughput.p95_series, throughput.p95_target_s, 0.001));
    let rpsRefY = $derived(throughput.rps_baseline > 0 ? throughput.rps_baseline / maxRps : undefined);
    let p95RefY = $derived(throughput.p95_target_s > 0 ? throughput.p95_target_s / maxP95 : undefined);
</script>

<div style="display:grid;grid-template-columns:1fr 1fr;gap:{D.pad}px">
    <Card {t} title="Requests / sec" sub="last 30 min{throughput.rps_baseline > 0 ? ` · baseline ${throughput.rps_baseline.toFixed(0)}` : ''}">
        <div style="padding:{D.pad + 2}px">
            <div style="display:flex;justify-content:space-between;font-size:{D.fz - 1}px;color:{t.muted};margin-bottom:6px">
                <span>RPS</span>
                <span style="font-family:ui-monospace,monospace;color:{t.ink};font-weight:600">{lastRps.toFixed(2)}</span>
            </div>
            <BarSeries
                series={throughput.rps_series}
                color={t.ok}
                height={72}
                referenceY={rpsRefY}
                refColor={t.warn}
                label="rps"
                formatValue={(v) => v.toFixed(2)}
                {t}
            />
        </div>
    </Card>

    <Card {t} title="Latency p95" sub="seconds · target < {throughput.p95_target_s}s">
        <div style="padding:{D.pad + 2}px">
            <div style="display:flex;justify-content:space-between;font-size:{D.fz - 1}px;color:{t.muted};margin-bottom:6px">
                <span>p95</span>
                <span style="font-family:ui-monospace,monospace;color:{t.ink};font-weight:600">{lastP95.toFixed(3)}<span style="color:{t.muted};font-weight:400">s</span></span>
            </div>
            <BarSeries
                series={throughput.p95_series}
                color={t.accent}
                height={72}
                referenceY={p95RefY}
                refColor={t.err}
                label="s"
                formatValue={(v) => v.toFixed(3)}
                {t}
            />
        </div>
    </Card>
</div>
