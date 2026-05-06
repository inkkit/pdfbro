<!-- src/lib/components/QueueWaitChart.svelte -->
<script lang="ts">
    import type { ThroughputPayload } from '$lib/types';
    import type { Theme } from '$lib/theme.svelte';
    import Card from './shared/Card.svelte';
    import BarSeries from './shared/BarSeries.svelte';

    let { throughput, t, D }: { throughput: ThroughputPayload; t: Theme; D: { pad: number; fz: number } } = $props();

    let lastWait = $derived(throughput.queue_wait_p95_series.at(-1) ?? 0);
    let tone = $derived(lastWait > 5000 ? t.err : lastWait > 1000 ? t.warn : t.ok);
</script>

<Card {t} title="Queue wait p95" sub="ms · time before processing starts">
    <div style="padding:{D.pad + 2}px">
        <div style="display:flex;justify-content:space-between;font-size:{D.fz - 1}px;color:{t.muted};margin-bottom:6px">
            <span>wait p95</span>
            <span style="font-family:ui-monospace,monospace;color:{tone};font-weight:600">
                {lastWait >= 1000 ? `${(lastWait / 1000).toFixed(1)}s` : `${lastWait.toFixed(0)}ms`}
            </span>
        </div>
        <BarSeries
            series={throughput.queue_wait_p95_series}
            color={tone}
            height={72}
            label="ms"
            formatValue={(v) => v >= 1000 ? `${(v/1000).toFixed(1)}s` : `${v.toFixed(0)}ms`}
            {t}
        />
    </div>
</Card>
