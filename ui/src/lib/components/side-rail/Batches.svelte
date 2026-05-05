<!-- src/lib/components/side-rail/Batches.svelte -->
<script lang="ts">
    import type { BatchPayload } from '$lib/types';
    import type { Theme } from '$lib/theme.svelte';
    import Card from '../shared/Card.svelte';
    import Pill from '../shared/Pill.svelte';
    import SlimBar from '../shared/SlimBar.svelte';

    let { batches, t, D, style = '' }: { batches: BatchPayload[]; t: Theme; D: { pad: number; fz: number; rowPy: number }; style?: string } = $props();

    function batchTone(status: string): 'ok' | 'warn' | 'err' | 'accent' | 'ink' {
        if (status === 'failed')    return 'err';
        if (status === 'completed') return 'ok';
        if (status === 'queued')    return 'ink';
        return 'accent';
    }

    // Only show active jobs in the scrollable list; completed/failed drop off automatically.
    let active     = $derived(batches.filter(b => b.status === 'running' || b.status === 'queued'));
    let queuedCnt  = $derived(batches.filter(b => b.status === 'queued').length);
    let runningCnt = $derived(batches.filter(b => b.status === 'running').length);
    let doneCnt    = $derived(batches.filter(b => b.status === 'completed' || b.status === 'failed').length);

    const SCROLL_H = 240; // px — fixed height for the scrollable list
</script>

<Card {t} title="Batches" sub="{queuedCnt} queued · {runningCnt} running · {doneCnt} done" {style}>
    {#if active.length === 0}
        <div style="padding:{D.pad}px;color:{t.muted};font-size:{D.fz}px">
            {doneCnt > 0 ? `${doneCnt} job${doneCnt > 1 ? 's' : ''} completed` : 'No active batches'}
        </div>
    {:else}
        <div style="overflow-y:auto;max-height:{SCROLL_H}px;font-family:ui-monospace,monospace;font-size:{D.fz - 0.5}px">
            {#each active as b, i}
                <div style="padding:{D.rowPy + 1}px {D.pad}px;{i < active.length - 1 ? `border-bottom:1px solid ${t.rule}` : ''}">
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
