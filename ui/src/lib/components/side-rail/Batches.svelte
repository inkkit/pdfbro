<!-- src/lib/components/side-rail/Batches.svelte -->
<script lang="ts">
    import type { BatchPayload } from '$lib/types';
    import type { Theme } from '$lib/theme.svelte';
    import Card from '../shared/Card.svelte';
    import Pill from '../shared/Pill.svelte';
    import SlimBar from '../shared/SlimBar.svelte';

    let { batches, t, D }: { batches: BatchPayload[]; t: Theme; D: { pad: number; fz: number; rowPy: number } } = $props();

    function batchTone(status: string): 'ok' | 'warn' | 'err' | 'accent' | 'ink' {
        if (status === 'failed') return 'err';
        if (status === 'completed') return 'ok';
        if (status === 'queued') return 'ink';
        return 'accent';
    }

    let activeCount = $derived(batches.filter(b => b.status === 'running' || b.status === 'queued').length);
</script>

<Card {t} title="Batches" sub="{activeCount} active">
    {#if batches.length === 0}
        <div style="padding:{D.pad}px;color:{t.muted};font-size:{D.fz}px">No recent batches</div>
    {:else}
        <div style="font-family:ui-monospace,monospace;font-size:{D.fz - 0.5}px">
            {#each batches as b, i}
                <div style="padding:{D.rowPy + 1}px {D.pad}px;{i < batches.length - 1 ? `border-bottom:1px solid ${t.rule}` : ''}">
                    <div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:3px">
                        <div style="display:flex;align-items:center;gap:6px">
                            <Pill tone={batchTone(b.status)} {t}>{b.status.slice(0, 4).toUpperCase()}</Pill>
                            <span style="color:{t.muted};font-size:9px">{b.id.slice(0, 16)}…</span>
                        </div>
                        <div style="display:flex;align-items:center;gap:6px">
                            <span style="color:{t.muted};font-size:9px;background:{t.faint};padding:1px 4px;border-radius:3px">{b.output_mode.toUpperCase()}</span>
                            <span style="color:{t.muted};font-size:9px">{b.elapsed}</span>
                        </div>
                    </div>
                    <div style="display:flex;align-items:center;gap:8px">
                        <div style="flex:1"><SlimBar pct={b.progress_pct} {t} h={4} /></div>
                        <span style="color:{t.muted};font-size:9px;white-space:nowrap">
                            {b.completed_items}/{b.total_items}
                            {#if b.failed_items > 0}<span style="color:{t.err}"> · {b.failed_items} err</span>{/if}
                        </span>
                    </div>
                </div>
            {/each}
        </div>
    {/if}
</Card>
