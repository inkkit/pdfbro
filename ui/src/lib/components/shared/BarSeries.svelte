<!-- Reusable interactive SVG bar chart with hover tooltip -->
<script lang="ts">
    import type { Theme } from '$lib/theme.svelte';

    let {
        series,
        color,
        height = 64,
        referenceY,       // 0–1 fraction where to draw reference line
        refColor,
        label = '',       // unit label shown in tooltip (e.g. "rps", "ms", "%")
        formatValue = (v: number) => v.toFixed(2),
        t,
    }: {
        series: number[];
        color: string;
        height?: number;
        referenceY?: number;
        refColor?: string;
        label?: string;
        formatValue?: (v: number) => string;
        t: Theme;
    } = $props();

    let hoveredIdx = $state<number | null>(null);
    let svgEl = $state<SVGSVGElement | null>(null);

    let max = $derived(Math.max(...series, 0.001));
    let hoveredValue = $derived(hoveredIdx !== null ? series[hoveredIdx] : null);

    function onMouseMove(e: MouseEvent) {
        if (!svgEl || series.length === 0) return;
        const rect = svgEl.getBoundingClientRect();
        const x = e.clientX - rect.left;
        const idx = Math.min(series.length - 1, Math.max(0, Math.floor((x / rect.width) * series.length)));
        hoveredIdx = idx;
    }

    function onMouseLeave() {
        hoveredIdx = null;
    }
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
        <!-- Reference line -->
        {#if referenceY !== undefined && referenceY > 0 && referenceY <= 1}
            {@const refPx = height - referenceY * height}
            <line
                x1="0" y1={refPx}
                x2="100%" y2={refPx}
                stroke={refColor ?? t.err}
                stroke-width="1"
                stroke-dasharray="3 3"
                opacity="0.55"
            />
        {/if}

        <!-- Bars -->
        {#each series as v, i}
            {@const w = 100 / series.length}
            {@const pct = Math.max(2 / height, v / max)}
            {@const barH = pct * height}
            {@const x = i * w}
            {@const isHovered = hoveredIdx === i}
            <rect
                x="{x + 0.3}%"
                y={height - barH}
                width="{w - 0.6}%"
                height={barH}
                fill={isHovered ? t.ink : color}
                opacity={isHovered ? 0.9 : 0.8}
                rx="1"
            />
        {/each}

        <!-- Hover cursor line -->
        {#if hoveredIdx !== null}
            {@const w = 100 / series.length}
            {@const cx = (hoveredIdx + 0.5) * w}
            <line
                x1="{cx}%" y1="0"
                x2="{cx}%" y2={height}
                stroke={t.muted}
                stroke-width="1"
                stroke-dasharray="2 2"
                opacity="0.4"
            />
        {/if}
    </svg>

    <!-- Tooltip -->
    {#if hoveredIdx !== null && hoveredValue !== null}
        {@const w = 100 / series.length}
        {@const pctLeft = (hoveredIdx + 0.5) * w}
        {@const flipLeft = pctLeft > 70}
        <div style="
            position:absolute;
            top:-28px;
            {flipLeft ? `right:${100 - pctLeft}%` : `left:${pctLeft}%`};
            transform:{flipLeft ? 'translateX(50%)' : 'translateX(-50%)'};
            background:{t.ink};
            color:{t.bg};
            font-family:ui-monospace,monospace;
            font-size:10px;
            padding:2px 6px;
            border-radius:3px;
            white-space:nowrap;
            pointer-events:none;
            z-index:10;
        ">
            {formatValue(hoveredValue)}{label ? ' ' + label : ''}
        </div>
    {/if}
</div>
