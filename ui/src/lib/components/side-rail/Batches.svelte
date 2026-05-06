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

    // Only show active jobs; completed/failed drop off automatically.
    let active     = $derived(batches.filter(b => b.status === 'running' || b.status === 'queued'));
    let queuedCnt  = $derived(batches.filter(b => b.status === 'queued').length);
    let runningCnt = $derived(batches.filter(b => b.status === 'running').length);
    let doneCnt    = $derived(batches.filter(b => b.status === 'completed' || b.status === 'failed').length);
</script>

<Card {t} title="Batches" sub="{queuedCnt} queued · {runningCnt} running · {doneCnt} done" {style}>
    {#if active.length === 0}
        <!-- Empty state -->
        <div style="display:flex;flex-direction:column;align-items:center;justify-content:center;padding:{D.pad * 3}px {D.pad}px;gap:10px">
            <svg width="48" height="48" viewBox="0 0 48 48" fill="none" xmlns="http://www.w3.org/2000/svg" style="opacity:0.25">
                <rect x="8" y="12" width="32" height="28" rx="3" stroke="{t.ink}" stroke-width="2" fill="none"/>
                <path d="M16 12V9a2 2 0 0 1 2-2h12a2 2 0 0 1 2 2v3" stroke="{t.ink}" stroke-width="2" stroke-linecap="round"/>
                <line x1="16" y1="22" x2="32" y2="22" stroke="{t.ink}" stroke-width="1.5" stroke-linecap="round"/>
                <line x1="16" y1="28" x2="28" y2="28" stroke="{t.ink}" stroke-width="1.5" stroke-linecap="round"/>
                <line x1="16" y1="34" x2="24" y2="34" stroke="{t.ink}" stroke-width="1.5" stroke-linecap="round"/>
            </svg>
            <div style="text-align:center">
                <div style="font-size:{D.fz}px;font-weight:500;color:{t.muted}">Queue is empty</div>
                {#if doneCnt > 0}
                    <div style="font-size:{D.fz - 1}px;color:{t.muted};margin-top:3px">{doneCnt} job{doneCnt > 1 ? 's' : ''} completed</div>
                {:else}
                    <div style="font-size:{D.fz - 1}px;color:{t.muted};margin-top:3px">POST /batch to submit a job</div>
                {/if}
            </div>
        </div>
    {:else}
        <div style="overflow-y:auto;font-family:ui-monospace,monospace;font-size:{D.fz - 0.5}px">
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
