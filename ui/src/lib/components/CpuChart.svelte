<!-- src/lib/components/CpuChart.svelte -->
<script lang="ts">
    import type { ResourcesPayload } from '$lib/types';
    import type { Theme } from '$lib/theme.svelte';
    import Card from './shared/Card.svelte';
    import BarSeries from './shared/BarSeries.svelte';

    let { resources, t, D }: { resources: ResourcesPayload; t: Theme; D: { pad: number; fz: number } } = $props();

    let last = $derived(resources.cpu_series.at(-1) ?? 0);
    let tone = $derived(last > 85 ? t.err : last > 60 ? t.warn : t.ok);
</script>

<Card {t} title="CPU" sub="% · cgroup-aware">
    <div style="padding:{D.pad + 2}px">
        <div style="display:flex;justify-content:space-between;font-size:{D.fz - 1}px;color:{t.muted};margin-bottom:6px">
            <span>usage</span>
            <span style="font-family:ui-monospace,monospace;color:{tone};font-weight:600">{last.toFixed(1)}<span style="color:{t.muted};font-weight:400">%</span></span>
        </div>
        <BarSeries
            series={resources.cpu_series}
            color={tone}
            height={72}
            label="%"
            formatValue={(v) => v.toFixed(1) + '%'}
            {t}
        />
    </div>
</Card>
