<!-- src/lib/components/side-rail/Engines.svelte -->
<script lang="ts">
    import type { EnginePayload } from '$lib/types';
    import type { Theme } from '$lib/theme.svelte';
    import Card from '../shared/Card.svelte';
    import Pill from '../shared/Pill.svelte';
    import BarSeries from '../shared/BarSeries.svelte';

    let { engines, convRps, t, D }: {
        engines: EnginePayload[];
        convRps: Record<string, number>;
        t: Theme;
        D: { fz: number; pad: number };
    } = $props();

    function engineTone(e: EnginePayload): 'ok' | 'warn' | 'err' | 'ink' {
        if (e.status === 'n/a') return 'ink';
        if (e.status !== 'up') return 'err';
        if (e.restarts > 5) return 'warn';
        return 'ok';
    }
    function engineColor(e: EnginePayload): string {
        const tone = engineTone(e);
        if (tone === 'ok') return t.ok;
        if (tone === 'warn') return t.warn;
        if (tone === 'err') return t.err;
        return t.muted;
    }
    function fmtIdle(s: number): string {
        if (s === 0) return 'active';
        if (s < 60) return `idle ${s}s`;
        return `idle ${Math.floor(s / 60)}m`;
    }
    function fmtBytes(mb: number): string {
        if (mb >= 1024) return `${(mb / 1024).toFixed(1)}GB`;
        return `${mb.toFixed(1)}MB`;
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
                        {#if (convRps[e.name.toLowerCase()] ?? 0) > 0}
                            <span style="font-family:ui-monospace,monospace;font-size:9px;color:{t.ok};background:{t.faint};padding:1px 5px;border-radius:3px">{(convRps[e.name.toLowerCase()] ?? 0).toFixed(2)} conv/s</span>
                        {/if}
                    </div>
                    <span style="color:{t.muted};font-size:10px;font-family:ui-monospace,monospace">
                        {e.restarts} activation{e.restarts !== 1 ? 's' : ''} · {e.mode}
                    </span>
                </div>
                {#if e.mini_series.length > 0}
                    <BarSeries
                        series={e.mini_series}
                        color={engineColor(e)}
                        height={28}
                        label="load"
                        formatValue={(v) => (v * 100).toFixed(0) + '%'}
                        {t}
                    />
                {/if}
                <div style="display:grid;grid-template-columns:1fr 1fr 1fr 1fr;gap:4px;margin-top:5px;font-size:9px;font-family:ui-monospace,monospace">
                    <div style="background:{t.faint};border-radius:3px;padding:3px 5px">
                        <div style="color:{t.muted}">total conv</div>
                        <div style="font-weight:600">{e.conversions_total}</div>
                    </div>
                    <div style="background:{t.faint};border-radius:3px;padding:3px 5px">
                        <div style="color:{t.muted}">err%</div>
                        <div style="font-weight:600;color:{e.error_rate > 1 ? t.err : e.error_rate > 0 ? t.warn : t.ok}">{e.error_rate.toFixed(2)}</div>
                    </div>
                    <div style="background:{t.faint};border-radius:3px;padding:3px 5px">
                        <div style="color:{t.muted}">data</div>
                        <div style="font-weight:600">{fmtBytes(e.bytes_mb)}</div>
                    </div>
                    <div style="background:{t.faint};border-radius:3px;padding:3px 5px">
                        <div style="color:{t.muted}">state</div>
                        <div style="font-weight:600;color:{e.idle_secs === 0 ? t.ok : t.muted}">{fmtIdle(e.idle_secs)}</div>
                    </div>
                </div>
            </div>
        {/each}
        {#if engines.length === 0}
            <div style="color:{t.muted}">No engines configured</div>
        {/if}
    </div>
</Card>
