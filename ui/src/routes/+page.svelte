<!-- ui/src/routes/+page.svelte -->
<script lang="ts">
    import { onMount, onDestroy } from 'svelte';
    import { metricsStore } from '$lib/metrics.svelte';
    import { themeStore } from '$lib/theme.svelte';
    import Header from '$lib/components/Header.svelte';
    import Ticker from '$lib/components/Ticker.svelte';
    import RoutesTable from '$lib/components/RoutesTable.svelte';
    import Engines from '$lib/components/side-rail/Engines.svelte';
    import Concurrency from '$lib/components/side-rail/Concurrency.svelte';
    import Batches from '$lib/components/side-rail/Batches.svelte';
    import Resources from '$lib/components/side-rail/Resources.svelte';
    import ThroughputStrip from '$lib/components/ThroughputStrip.svelte';
    import ActivityStrip from '$lib/components/ActivityStrip.svelte';

    onMount(() => metricsStore.start());
    onDestroy(() => metricsStore.stop());

    let tweaksOpen = $state(false);

    const ACCENTS = [
        { label: 'Blue',   value: '#4f8ef7' },
        { label: 'Violet', value: '#8b5cf6' },
        { label: 'Teal',   value: '#14b8a6' },
        { label: 'Orange', value: '#f97316' },
        { label: 'Rose',   value: '#f43f5e' },
    ];

    let t  = $derived(themeStore.theme);
    let D  = $derived(themeStore.D);
</script>

<div style="background:{t.bg};color:{t.ink};font-family:'Geist Variable',ui-sans-serif,system-ui,sans-serif;min-height:100vh;padding:{D.gap + 4}px;transition:background 0.25s ease,color 0.25s ease">
    {#if metricsStore.loading}
        <div style="display:flex;align-items:center;justify-content:center;height:80vh;color:{t.muted}">
            Connecting to Folio…
        </div>
    {:else if metricsStore.data}
        <!-- Header -->
        <Header data={metricsStore.data} {t} />

        <!-- Ticker -->
        <div style="margin-top:{D.gap}px">
            <Ticker ticker={metricsStore.data.ticker} {t} {D} />
        </div>

        <!-- Main split: routes (8fr) + side rail (4fr) -->
        <div style="display:grid;grid-template-columns:8fr 4fr;gap:{D.gap}px;margin-top:{D.gap}px">
            <RoutesTable routes={metricsStore.data.routes} {t} {D} />
            <div style="display:flex;flex-direction:column;gap:{D.gap}px">
                <Engines engines={metricsStore.data.engines} {t} {D} />
                <Concurrency conc={metricsStore.data.concurrency} {t} {D} />
                <Batches batches={metricsStore.data.batches} {t} {D} />
                <Resources resources={metricsStore.data.resources} {t} {D} />
            </div>
        </div>

        <!-- Throughput strip -->
        <div style="margin-top:{D.gap}px">
            <ThroughputStrip throughput={metricsStore.data.throughput} {t} {D} />
        </div>

        <!-- Activity -->
        <div style="margin-top:{D.gap}px">
            <ActivityStrip requests={metricsStore.data.recent_requests} errors={metricsStore.data.recent_errors} {t} {D} />
        </div>
    {/if}
</div>

<!-- Tweaks panel (fixed bottom-right) -->
<div style="position:fixed;bottom:16px;right:16px;z-index:50">
    {#if tweaksOpen}
        <div style="background:{t.surface};border:1px solid {t.rule};border-radius:12px;padding:12px 16px;margin-bottom:8px;width:200px;display:flex;flex-direction:column;gap:10px;font-size:11px">
            <!-- Theme toggle -->
            <div>
                <div style="color:{t.muted};font-size:10px;text-transform:uppercase;letter-spacing:0.05em;margin-bottom:4px">Theme</div>
                <div style="display:flex;gap:6px">
                    {#each [['Light', false], ['Dark', true]] as [label, val]}
                        <button
                            onclick={() => { themeStore.dark = val as boolean; }}
                            style="flex:1;padding:3px 0;border:1px solid {themeStore.dark === val ? t.ink : t.rule};border-radius:6px;background:{themeStore.dark === val ? t.ink : 'transparent'};color:{themeStore.dark === val ? t.bg : t.ink};font-size:10.5px;cursor:pointer"
                        >{label}</button>
                    {/each}
                </div>
            </div>
            <!-- Accent swatches -->
            <div>
                <div style="color:{t.muted};font-size:10px;text-transform:uppercase;letter-spacing:0.05em;margin-bottom:4px">Accent</div>
                <div style="display:flex;gap:5px">
                    {#each ACCENTS as a}
                        <button
                            onclick={() => { themeStore.accent = a.value; }}
                            style="width:20px;height:20px;border-radius:999px;background:{a.value};border:2px solid {themeStore.accent === a.value ? t.ink : 'transparent'};cursor:pointer"
                            title={a.label}
                        ></button>
                    {/each}
                </div>
            </div>
            <!-- Density -->
            <div>
                <div style="color:{t.muted};font-size:10px;text-transform:uppercase;letter-spacing:0.05em;margin-bottom:4px">Density</div>
                <div style="display:flex;gap:4px">
                    {#each ['compact', 'regular', 'comfy'] as d}
                        <button
                            onclick={() => { themeStore.density = d as 'compact' | 'regular' | 'comfy'; }}
                            style="flex:1;padding:2px 0;border:1px solid {themeStore.density === d ? t.ink : t.rule};border-radius:5px;background:{themeStore.density === d ? t.ink : 'transparent'};color:{themeStore.density === d ? t.bg : t.ink};font-size:10px;cursor:pointer"
                        >{d.slice(0,1).toUpperCase() + d.slice(1)}</button>
                    {/each}
                </div>
            </div>
        </div>
    {/if}
    <button
        onclick={() => { tweaksOpen = !tweaksOpen; }}
        style="background:{t.surface};border:1px solid {t.rule};border-radius:9px;padding:6px 12px;font-size:11px;color:{t.muted};cursor:pointer;display:block;margin-left:auto"
    >
        ⚙ tweaks
    </button>
</div>
