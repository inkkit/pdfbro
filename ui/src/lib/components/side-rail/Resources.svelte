<!-- src/lib/components/side-rail/Resources.svelte -->
<script lang="ts">
    import type { ResourcesPayload } from '$lib/types';
    import type { Theme } from '$lib/theme.svelte';
    import Card from '../shared/Card.svelte';
    import BarSeries from '../shared/BarSeries.svelte';

    let { resources, t, D }: { resources: ResourcesPayload; t: Theme; D: { pad: number; fz: number } } = $props();

    let lastCpu = $derived(resources.cpu_series.at(-1) ?? 0);
    let lastMem = $derived(resources.memory_series.at(-1) ?? 0);
</script>

<Card {t} title="Resources" sub="last 30 min">
    <div style="padding:{D.pad + 2}px;display:flex;flex-direction:column;gap:10px">
        <div>
            <div style="display:flex;justify-content:space-between;font-size:{D.fz - 1}px;color:{t.muted};margin-bottom:4px">
                <span>CPU</span>
                <span style="font-family:ui-monospace,monospace;color:{t.ink};font-weight:600">{lastCpu.toFixed(1)}<span style="color:{t.muted};font-weight:400">%</span></span>
            </div>
            <BarSeries
                series={resources.cpu_series}
                color="var(--chart-1)"
                height={44}
                label="%"
                formatValue={(v) => v.toFixed(1)}
                {t}
            />
        </div>
        <div style="height:1px;background:{t.rule}"></div>
        <div>
            <div style="display:flex;justify-content:space-between;font-size:{D.fz - 1}px;color:{t.muted};margin-bottom:4px">
                <span>Memory</span>
                <span style="font-family:ui-monospace,monospace;color:{t.ink};font-weight:600">
                    {(lastMem / 1024).toFixed(2)}<span style="color:{t.muted};font-weight:400"> GB{resources.memory_max_mb > 0 ? ` / ${(resources.memory_max_mb / 1024).toFixed(0)} GB` : ''}</span>
                </span>
            </div>
            <BarSeries
                series={resources.memory_series}
                color="var(--chart-2)"
                height={44}
                referenceY={resources.memory_max_mb > 0 ? (resources.memory_series.at(-1) ?? 0) / resources.memory_max_mb : undefined}
                refColor={t.warn}
                label="MB"
                formatValue={(v) => (v / 1024).toFixed(2) + ' GB'}
                {t}
            />
        </div>
    </div>
</Card>
