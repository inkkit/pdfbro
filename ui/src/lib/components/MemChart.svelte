<!-- src/lib/components/MemChart.svelte -->
<script lang="ts">
    import type { ResourcesPayload } from '$lib/types';
    import type { Theme } from '$lib/theme.svelte';
    import Card from './shared/Card.svelte';
    import BarSeries from './shared/BarSeries.svelte';

    let { resources, t, D }: { resources: ResourcesPayload; t: Theme; D: { pad: number; fz: number } } = $props();

    let last = $derived(resources.memory_series.at(-1) ?? 0);
    let pct  = $derived(resources.memory_max_mb > 0 ? last / resources.memory_max_mb : 0);
    let tone = $derived(pct > 0.85 ? t.err : pct > 0.60 ? t.warn : t.ok);
</script>

<Card {t} title="Memory" sub="GB · container limit">
    <div style="padding:{D.pad + 2}px">
        <div style="display:flex;justify-content:space-between;font-size:{D.fz - 1}px;color:{t.muted};margin-bottom:6px">
            <span>usage</span>
            <span style="font-family:ui-monospace,monospace;color:{tone};font-weight:600">
                {(last / 1024).toFixed(2)}<span style="color:{t.muted};font-weight:400"> GB{resources.memory_max_mb > 0 ? ` / ${(resources.memory_max_mb / 1024).toFixed(0)} GB` : ''}</span>
            </span>
        </div>
        <BarSeries
            series={resources.memory_series}
            color={tone}
            height={72}
            referenceY={resources.memory_max_mb > 0 ? pct : undefined}
            refColor={t.warn}
            label="MB"
            formatValue={(v) => (v / 1024).toFixed(2) + ' GB'}
            {t}
        />
    </div>
</Card>
