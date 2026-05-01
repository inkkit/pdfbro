<!-- src/lib/components/side-rail/Resources.svelte -->
<script lang="ts">
    import type { ResourcesPayload } from '$lib/types';
    import type { Theme } from '$lib/theme.svelte';
    import Card from '../shared/Card.svelte';

    let { resources, t, D }: { resources: ResourcesPayload; t: Theme; D: { pad: number } } = $props();

    let lastCpu = $derived(resources.cpu_series.at(-1) ?? 0);
    let lastMem = $derived(resources.memory_series.at(-1) ?? 0);
    let maxCpu = $derived(Math.max(1, ...resources.cpu_series));
    let maxMem = $derived(Math.max(1, ...resources.memory_series));
</script>

<Card {t} title="Resources" sub="last 30 min">
    <div style="padding:{D.pad + 2}px;display:flex;flex-direction:column;gap:10px">
        <!-- CPU -->
        <div>
            <div style="display:flex;justify-content:space-between;font-size:10.5px;color:{t.muted};margin-bottom:4px">
                <span>CPU</span>
                <span style="font-family:ui-monospace,monospace;color:{t.ink};font-weight:600">{lastCpu.toFixed(0)}<span style="color:{t.muted};font-weight:400">%</span></span>
            </div>
            <div style="display:flex;align-items:flex-end;gap:1px;height:48px;background:{t.faint};border-radius:4px;padding:2px;overflow:hidden">
                {#each resources.cpu_series as v}
                    <div style="flex:1;min-width:1px;background:var(--chart-1);height:{Math.max(2, (v / maxCpu) * 100)}%;border-radius:1px 1px 0 0"></div>
                {/each}
            </div>
        </div>
        <div style="height:1px;background:{t.rule}"></div>
        <!-- Memory -->
        <div>
            <div style="display:flex;justify-content:space-between;font-size:10.5px;color:{t.muted};margin-bottom:4px">
                <span>Memory</span>
                <span style="font-family:ui-monospace,monospace;color:{t.ink};font-weight:600">
                    {(lastMem / 1024).toFixed(2)}<span style="color:{t.muted};font-weight:400"> GB{resources.memory_max_mb > 0 ? ` / ${(resources.memory_max_mb / 1024).toFixed(0)} GB` : ''}</span>
                </span>
            </div>
            <div style="display:flex;align-items:flex-end;gap:1px;height:48px;background:{t.faint};border-radius:4px;padding:2px;overflow:hidden">
                {#each resources.memory_series as v}
                    <div style="flex:1;min-width:1px;background:var(--chart-2);height:{Math.max(2, (v / maxMem) * 100)}%;border-radius:1px 1px 0 0"></div>
                {/each}
            </div>
        </div>
    </div>
</Card>
