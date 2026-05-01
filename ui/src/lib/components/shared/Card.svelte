<!-- src/lib/components/shared/Card.svelte -->
<script lang="ts">
    import type { Theme } from '$lib/theme.svelte';

    let { title, sub, t, style = '', children }: {
        title?: string;
        sub?: string;
        t: Theme;
        style?: string;
        children: import('svelte').Snippet;
    } = $props();
</script>

<div style="background:{t.surface};border:1px solid {t.rule};border-radius:6px;overflow:hidden;{style}">
    {#if title}
        <div style="display:flex;align-items:baseline;gap:8px;padding:10px 14px;border-bottom:1px solid {t.rule}">
            <span style="font-size:11.5px;font-weight:600;letter-spacing:0.02em;color:{t.ink}">{title}</span>
            {#if sub}<span style="font-size:10.5px;color:{t.muted}">{sub}</span>{/if}
        </div>
    {/if}
    {@render children()}
</div>
