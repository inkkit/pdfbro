<!-- src/lib/components/EngineConvChart.svelte -->
<script lang="ts">
    import type { ThroughputPayload } from '$lib/types';
    import type { Theme } from '$lib/theme.svelte';
    import Card from './shared/Card.svelte';
    import StackedBarSeries from './StackedBarSeries.svelte';

    let { throughput, t, D }: { throughput: ThroughputPayload; t: Theme; D: { pad: number; fz: number } } = $props();

    let lastCh = $derived(throughput.chromium_conv_series.at(-1) ?? 0);
    let lastLo = $derived(throughput.libreoffice_conv_series.at(-1) ?? 0);
</script>

<Card {t} title="Engine conversions" sub="conv/sec · stacked">
    <div style="padding:{D.pad + 2}px">
        <div style="display:flex;justify-content:space-between;font-size:{D.fz - 1}px;color:{t.muted};margin-bottom:6px">
            <div style="display:flex;gap:10px">
                <span><span style="color:#14b8a6">■</span> Chromium <strong style="color:{t.ink};font-family:ui-monospace,monospace">{lastCh.toFixed(2)}</strong></span>
                <span><span style="color:#f59e0b">■</span> LibreOffice <strong style="color:{t.ink};font-family:ui-monospace,monospace">{lastLo.toFixed(2)}</strong></span>
            </div>
        </div>
        <StackedBarSeries
            seriesA={throughput.chromium_conv_series}
            seriesB={throughput.libreoffice_conv_series}
            colorA="#14b8a6"
            colorB="#f59e0b"
            labelA="Chromium"
            labelB="LibreOffice"
            height={72}
            {t}
        />
    </div>
</Card>
