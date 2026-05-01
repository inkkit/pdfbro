<!-- ui/src/lib/components/ActivityStrip.svelte -->
<script lang="ts">
    import type { RequestLogEntry, ErrorLogEntry } from '$lib/types';
    import type { Theme } from '$lib/theme.svelte';
    import Card from './shared/Card.svelte';

    let { requests, errors, t, D }: {
        requests: RequestLogEntry[];
        errors: ErrorLogEntry[];
        t: Theme;
        D: { pad: number; fz: number; rowPy: number };
    } = $props();

    function statusColor(code: number): string {
        if (code >= 500) return t.err;
        if (code >= 400) return t.warn;
        return t.muted;
    }
</script>

<div style="display:grid;grid-template-columns:1fr 1fr;gap:{D.pad}px">
    <Card {t} title="Requests" sub="latest first">
        <div style="font-family:ui-monospace,monospace;font-size:{D.fz - 0.5}px;overflow:hidden">
            {#each requests as r, i}
                <div style="display:grid;grid-template-columns:60px 40px 1fr 38px 52px;align-items:center;gap:6px;padding:{D.rowPy + 2}px {D.pad + 4}px;{i < requests.length - 1 ? `border-bottom:1px solid ${t.rule}` : ''}">
                    <span style="color:{t.muted}">{r.time}</span>
                    <span style="color:{t.muted}">{r.method}</span>
                    <span style="overflow:hidden;text-overflow:ellipsis;white-space:nowrap">{r.route}</span>
                    <span style="color:{statusColor(r.status)};font-weight:600;text-align:right">{r.status}</span>
                    <span style="text-align:right;color:{t.muted}">{r.duration_ms}ms</span>
                </div>
            {/each}
            {#if requests.length === 0}
                <div style="padding:{D.pad}px;color:{t.muted}">No requests yet</div>
            {/if}
        </div>
    </Card>

    <Card {t} title="Errors" sub="latest first">
        <div style="font-family:ui-monospace,monospace;font-size:{D.fz - 0.5}px;overflow:hidden">
            {#each errors as e, i}
                <div style="display:grid;grid-template-columns:60px 1fr auto;align-items:start;gap:6px;padding:{D.rowPy + 2}px {D.pad + 4}px;{i < errors.length - 1 ? `border-bottom:1px solid ${t.rule}` : ''}">
                    <span style="color:{t.muted}">{e.time}</span>
                    <span style="overflow:hidden;text-overflow:ellipsis;white-space:nowrap;color:{t.err}">{e.message}</span>
                    <span style="color:{t.muted};white-space:nowrap">{e.route}</span>
                </div>
            {/each}
            {#if errors.length === 0}
                <div style="padding:{D.pad}px;color:{t.muted}">No errors</div>
            {/if}
        </div>
    </Card>
</div>
