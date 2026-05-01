<!-- src/lib/components/side-rail/Engines.svelte -->
<script lang="ts">
    import type { EnginePayload } from '$lib/types';
    import type { Theme } from '$lib/theme.svelte';
    import Card from '../shared/Card.svelte';
    import Pill from '../shared/Pill.svelte';
    import BarSeries from '../shared/BarSeries.svelte';

    let { engines, t, D }: { engines: EnginePayload[]; t: Theme; D: { fz: number; pad: number } } = $props();

    function engineTone(e: EnginePayload): 'ok' | 'warn' | 'err' | 'ink' {
        if (e.status === 'n/a') return 'ink';
        if (e.status !== 'up') return 'err';
        if (e.restarts > 5) return 'warn';
        return 'ok';
    }
    function engineColor(e: EnginePayload, t: Theme): string {
        const tone = engineTone(e);
        if (tone === 'ok') return t.ok;
        if (tone === 'warn') return t.warn;
        if (tone === 'err') return t.err;
        return t.muted;
    }
</script>

<Card {t} title="Engines">
    <div style="padding:{D.pad}px;font-size:{D.fz}px">
        {#each engines as e, i}
            <div style="{i > 0 ? `margin-top:${D.pad - 2}px;padding-top:${D.pad - 2}px;border-top:1px solid ${t.rule}` : ''}">
                <div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:4px">
                    <div style="display:flex;align-items:center;gap:6px">
                        <strong style="font-size:{D.fz + 0.5}px">{e.name}</strong>
                        <Pill tone={engineTone(e)} {t}>{e.status.toUpperCase()}</Pill>
                    </div>
                    <span style="color:{t.muted};font-size:10px;font-family:ui-monospace,monospace">
                        {e.restarts} activation{e.restarts !== 1 ? 's' : ''} · {e.mode}
                    </span>
                </div>
                {#if e.mini_series.length > 0}
                    <BarSeries
                        series={e.mini_series}
                        color={engineColor(e, t)}
                        height={28}
                        label="load"
                        formatValue={(v) => (v * 100).toFixed(0) + '%'}
                        {t}
                    />
                {/if}
            </div>
        {/each}
        {#if engines.length === 0}
            <div style="color:{t.muted}">No engines configured</div>
        {/if}
    </div>
</Card>
