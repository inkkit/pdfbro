<!-- src/lib/components/side-rail/Engines.svelte -->
<script lang="ts">
    import type { EnginePayload } from '$lib/types';
    import type { Theme } from '$lib/theme.svelte';
    import Card from '../shared/Card.svelte';
    import Pill from '../shared/Pill.svelte';

    let { engines, t, D }: { engines: EnginePayload[]; t: Theme; D: { fz: number; pad: number } } = $props();

    function engineTone(e: EnginePayload): 'ok' | 'warn' | 'err' | 'ink' {
        if (e.status === 'n/a') return 'ink';
        if (e.status !== 'up') return 'err';
        if (e.restarts > 5) return 'warn';
        return 'ok';
    }
</script>

<Card {t} title="Engines">
    <div style="padding:{D.pad}px;font-size:{D.fz}px">
        {#each engines as e, i}
            <div style="display:grid;grid-template-columns:1fr auto;align-items:center;gap:8px;{i > 0 ? `margin-top:${D.pad - 2}px;padding-top:${D.pad - 2}px;border-top:1px solid ${t.rule}` : ''}">
                <div>
                    <div style="display:flex;align-items:center;gap:6px">
                        <strong style="font-size:{D.fz + 0.5}px">{e.name}</strong>
                        <Pill tone={engineTone(e)} {t}>{e.status.toUpperCase()}</Pill>
                    </div>
                    <div style="color:{t.muted};font-size:10.5px;margin-top:2px;font-family:ui-monospace,monospace">
                        {e.restarts} restart{e.restarts !== 1 ? 's' : ''} · {e.mode}
                    </div>
                </div>
                {#if e.mini_series.length > 0}
                    <div style="display:flex;align-items:flex-end;gap:1px;height:28px;width:60px">
                        {#each e.mini_series as v}
                            {@const barColor = engineTone(e) === 'ok' ? t.ok : engineTone(e) === 'warn' ? t.warn : t.err}
                            <div style="flex:1;background:{barColor};opacity:0.7;height:{Math.max(4, Math.round(v * 100))}%;border-radius:1px 1px 0 0"></div>
                        {/each}
                    </div>
                {/if}
            </div>
        {/each}
        {#if engines.length === 0}
            <div style="color:{t.muted}">No engines configured</div>
        {/if}
    </div>
</Card>
