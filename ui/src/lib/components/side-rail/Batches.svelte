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
</script>

<Card {t} title="Batches" sub="{batches.filter(b => b.status === 'running').length} active">
    {#if batches.length === 0}
        <div style="padding:{D.pad}px;color:{t.muted};font-size:{D.fz}px">No recent batches</div>
    {:else}
        <table style="width:100%;border-collapse:collapse;font-family:ui-monospace,monospace;font-size:{D.fz - 0.5}px">
            <tbody>
                {#each batches as b, i}
                    <tr style="{i < batches.length - 1 ? `border-bottom:1px solid ${t.rule}` : ''}">
                        <td style="padding:{D.rowPy + 1}px {D.pad}px">{b.id}</td>
                        <td style="padding:{D.rowPy + 1}px 4px"><Pill tone={batchTone(b.status)} {t}>{b.status.slice(0, 4)}</Pill></td>
                        <td style="padding:{D.rowPy + 1}px 4px;width:70px"><SlimBar pct={b.progress_pct} {t} h={4} /></td>
                        <td style="padding:{D.rowPy + 1}px {D.pad}px;text-align:right;color:{t.muted}">{b.elapsed}</td>
                    </tr>
                {/each}
            </tbody>
        </table>
    {/if}
</Card>
