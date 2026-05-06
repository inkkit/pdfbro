<!-- src/lib/components/side-rail/Concurrency.svelte -->
<script lang="ts">
    import type { ConcurrencyPayload } from '$lib/types';
    import type { Theme } from '$lib/theme.svelte';
    import Card from '../shared/Card.svelte';
    import Pill from '../shared/Pill.svelte';

    let { conc, t, D }: { conc: ConcurrencyPayload; t: Theme; D: { pad: number; fz: number } } = $props();

    let tone = $derived((conc.active >= conc.crit_threshold ? 'warn' : conc.active >= conc.warn_threshold ? 'warn' : 'ok') as 'ok' | 'warn' | 'err');
    let pct = $derived(Math.round((conc.active / Math.max(1, conc.max)) * 100));
    let statusLabel = $derived(conc.active > conc.max ? 'BUSY' : tone === 'warn' ? 'WARN' : 'OK');

    let hoveredSlot = $state<number | null>(null);

    function slotColor(i: number): string {
        const filled = i < conc.active;
        if (!filled) return t.faint;
        if (i >= conc.crit_threshold) return t.err;
        if (i >= conc.warn_threshold) return t.warn;
        return t.ok;
    }

    function slotLabel(i: number): string {
        const filled = i < conc.active;
        const state = filled ? 'active' : 'free';
        const zone = i >= conc.crit_threshold ? ' · crit zone' : i >= conc.warn_threshold ? ' · warn zone' : '';
        return `Slot ${i + 1}: ${state}${zone}`;
    }
</script>

<Card {t} title="Concurrency" sub="semaphore · {conc.max} slots">
    <div style="padding:{D.pad}px;font-size:{D.fz}px">
        <div style="display:flex;align-items:baseline;justify-content:space-between;margin-bottom:8px">
            <div style="font-family:ui-monospace,monospace;font-size:26px;font-weight:600;letter-spacing:-0.01em">
                {conc.active}<span style="color:{t.muted};font-weight:400"> / {conc.max}</span>
            </div>
            <Pill {tone} {t}>{Math.min(pct, 100)}% · {statusLabel}</Pill>
        </div>

        <!-- Slot grid -->
        <div style="position:relative">
            <div style="display:grid;grid-template-columns:repeat(32,1fr);gap:2px">
                {#each Array.from({ length: conc.max }, (_, i) => i) as i}
                    <!-- svelte-ignore a11y_no_static_element_interactions -->
                    <div
                        style="height:12px;background:{slotColor(i)};border-radius:2px;cursor:default;{hoveredSlot === i ? `outline:1px solid ${t.ink};outline-offset:1px` : ''}"
                        onmouseenter={() => hoveredSlot = i}
                        onmouseleave={() => hoveredSlot = null}
                    ></div>
                {/each}
            </div>
            <!-- Tooltip -->
            {#if hoveredSlot !== null}
                <div style="
                    position:absolute;
                    bottom:calc(100% + 6px);
                    left:50%;
                    transform:translateX(-50%);
                    background:{t.ink};
                    color:{t.bg};
                    font-family:ui-monospace,monospace;
                    font-size:10px;
                    padding:2px 7px;
                    border-radius:3px;
                    white-space:nowrap;
                    pointer-events:none;
                    z-index:10;
                ">
                    {slotLabel(hoveredSlot)}
                </div>
            {/if}
        </div>

        <div style="display:flex;justify-content:space-between;margin-top:6px;font-family:ui-monospace,monospace;font-size:10px;color:{t.muted}">
            <span>0</span><span>warn {conc.warn_threshold}</span><span>crit {conc.crit_threshold}</span><span>{conc.max}</span>
        </div>

        <!-- Queue stats row -->
        <div style="display:grid;grid-template-columns:1fr 1fr;gap:6px;margin-top:8px">
            <div style="background:{t.faint};border-radius:4px;padding:4px 8px;font-size:10px;font-family:ui-monospace,monospace">
                <div style="color:{t.muted};font-size:9px;text-transform:uppercase;letter-spacing:0.04em">wait p95</div>
                <div style="font-weight:600;color:{conc.queue_wait_p95_ms > 5000 ? t.err : conc.queue_wait_p95_ms > 1000 ? t.warn : t.ok}">
                    {conc.queue_wait_p95_ms >= 1000 ? `${(conc.queue_wait_p95_ms / 1000).toFixed(1)}s` : `${conc.queue_wait_p95_ms.toFixed(0)}ms`}
                </div>
                <div style="color:{t.muted};font-size:8px;margin-top:1px">time before slot acquired</div>
            </div>
            <div style="background:{t.faint};border-radius:4px;padding:4px 8px;font-size:10px;font-family:ui-monospace,monospace">
                <div style="color:{t.muted};font-size:9px;text-transform:uppercase;letter-spacing:0.04em">processing</div>
                <div style="font-weight:600">{conc.queue_processing} job{conc.queue_processing !== 1 ? 's' : ''}</div>
            </div>
        </div>
    </div>
</Card>
