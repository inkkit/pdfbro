<!-- src/lib/components/shared/Pill.svelte -->
<script lang="ts">
    import type { Theme } from '$lib/theme.svelte';

    let { tone = 'ink', t, children }: {
        tone?: 'ok' | 'warn' | 'err' | 'accent' | 'ink';
        t: Theme;
        children: import('svelte').Snippet;
    } = $props();

    let color = $derived(t[tone as keyof Theme] as string ?? t.ink);
</script>

<span style="
    color:{color};
    background:{color}22;
    padding:1px 7px;
    font-family:ui-monospace,monospace;
    font-size:10px;
    font-weight:600;
    border-radius:999px;
    letter-spacing:0.04em;
    display:inline-block;
    line-height:16px;
    text-transform:uppercase;
">
    {@render children()}
</span>
