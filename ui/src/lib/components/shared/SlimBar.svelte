<!-- src/lib/components/shared/SlimBar.svelte -->
<script lang="ts">
    import type { Theme } from '$lib/theme.svelte';

    let { pct, t, h = 6 }: { pct: number; t: Theme; h?: number } = $props();
    let color = $derived(pct > 85 ? t.err : pct > 60 ? t.warn : t.ok);
</script>

<div style="position:relative;height:{h}px;background:{t.faint};border-radius:999px;overflow:hidden">
    <div style="height:100%;width:{Math.min(100, pct)}%;background:{color};border-radius:999px;transition:width 0.4s ease"></div>
</div>
