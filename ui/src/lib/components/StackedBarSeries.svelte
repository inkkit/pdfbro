<!-- src/lib/components/StackedBarSeries.svelte -->
<script lang="ts">
    import type { Theme } from '$lib/theme.svelte';

    let {
        seriesA,
        seriesB,
        colorA,
        colorB,
        height = 64,
        labelA = 'A',
        labelB = 'B',
        t,
    }: {
        seriesA: number[];
        seriesB: number[];
        colorA: string;
        colorB: string;
        height?: number;
        labelA?: string;
        labelB?: string;
        t: Theme;
    } = $props();

    let len = $derived(Math.max(seriesA.length, seriesB.length));
    let combined = $derived(
        Array.from({ length: len }, (_, i) => (seriesA[i] ?? 0) + (seriesB[i] ?? 0))
    );
    let maxVal = $derived(Math.max(...combined, 0.001));

    let hoveredIdx = $state<number | null>(null);
    let svgEl = $state<SVGSVGElement | null>(null);

    function onMouseMove(e: MouseEvent) {
        if (!svgEl || len === 0) return;
        const rect = svgEl.getBoundingClientRect();
        const x = e.clientX - rect.left;
        hoveredIdx = Math.min(len - 1, Math.max(0, Math.floor((x / rect.width) * len)));
    }
    function onMouseLeave() { hoveredIdx = null; }
</script>

<div style="position:relative;width:100%;height:{height}px">
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <svg
        bind:this={svgEl}
        width="100%"
        height={height}
        style="display:block;overflow:visible"
        onmousemove={onMouseMove}
        onmouseleave={onMouseLeave}
    >
        {#each Array.from({ length: len }, (_, i) => i) as i}
            {@const w = 100 / len}
            {@const a = seriesA[i] ?? 0}
            {@const b = seriesB[i] ?? 0}
            {@const total = a + b}
            {@const totalH = (total / maxVal) * height}
            {@const aH = total > 0 ? (a / total) * totalH : 0}
            {@const bH = totalH - aH}
            {@const x = i * w}
            <!-- Series A (bottom) -->
            <rect
                x="{x + 0.3}%"
                y={height - aH}
                width="{w - 0.6}%"
                height={aH}
                fill={colorA}
                opacity={hoveredIdx === i ? 1 : 0.8}
                rx="1"
            />
            <!-- Series B (top) -->
            {#if bH > 0}
                <rect
                    x="{x + 0.3}%"
                    y={height - totalH}
                    width="{w - 0.6}%"
                    height={bH}
                    fill={colorB}
                    opacity={hoveredIdx === i ? 1 : 0.8}
                    rx="1"
                />
            {/if}
        {/each}
        {#if hoveredIdx !== null}
            {@const w = 100 / len}
            {@const cx = (hoveredIdx + 0.5) * w}
            <line x1="{cx}%" y1="0" x2="{cx}%" y2={height}
                stroke={t.muted} stroke-width="1" stroke-dasharray="2 2" opacity="0.4" />
        {/if}
    </svg>
    {#if hoveredIdx !== null}
        {@const w = 100 / len}
        {@const pctLeft = (hoveredIdx + 0.5) * w}
        {@const flipLeft = pctLeft > 70}
        {@const a = (seriesA[hoveredIdx] ?? 0).toFixed(2)}
        {@const b = (seriesB[hoveredIdx] ?? 0).toFixed(2)}
        <div style="
            position:absolute;top:-34px;
            {flipLeft ? `right:${100 - pctLeft}%` : `left:${pctLeft}%`};
            transform:{flipLeft ? 'translateX(50%)' : 'translateX(-50%)'};
            background:{t.ink};color:{t.bg};
            font-family:ui-monospace,monospace;font-size:10px;
            padding:2px 7px;border-radius:3px;white-space:nowrap;pointer-events:none;z-index:10
        ">
            <span style="color:{colorA}">{labelA} {a}</span>
            &nbsp;·&nbsp;
            <span style="color:{colorB}">{labelB} {b}</span>
        </div>
    {/if}
</div>
