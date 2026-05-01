<!-- src/lib/components/Header.svelte -->
<script lang="ts">
    import type { ConsolePayload } from '$lib/types';
    import type { Theme } from '$lib/theme.svelte';
    import Pill from './shared/Pill.svelte';
    import { lastRefreshed, manualRefresh, connected } from '$lib/metrics.svelte';

    let { data, t }: { data: ConsolePayload; t: Theme } = $props();

    let refreshed = $derived(lastRefreshed
        ? `${lastRefreshed.toLocaleTimeString('en-GB')} · refreshed`
        : 'connecting…'
    );
</script>

<div style="background:{t.surface};border:1px solid {t.rule};border-radius:12px;padding:8px 14px;display:flex;align-items:center;gap:12px;font-size:11.5px">
    <span style="font-weight:700;font-size:14px;letter-spacing:-0.01em">Folio</span>
    <span style="color:{t.muted};font-family:ui-monospace,monospace;font-size:10.5px">v{data.version}</span>
    <Pill tone="accent" {t}>prod</Pill>
    <Pill tone={connected ? 'ok' : 'err'} {t}>● {connected ? 'live' : 'disconnected'}</Pill>
    <span style="flex:1"></span>
    <span style="color:{t.muted};font-family:ui-monospace,monospace;font-size:10.5px">{refreshed}</span>
    <button
        onclick={manualRefresh}
        style="border:1px solid {t.rule};background:transparent;color:{t.ink};padding:3px 9px;border-radius:7px;font-family:ui-monospace,monospace;font-size:10.5px;cursor:pointer"
    >
        ↺ refresh
    </button>
</div>
