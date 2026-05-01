// src/lib/metrics.svelte.ts
import type { ConsolePayload } from './types';

export let data = $state<ConsolePayload | null>(null);
export let loading = $state(true);
export let connected = $state(false);
export let error = $state<string | null>(null);
export let lastRefreshed = $state<Date | null>(null);

let es: EventSource | null = null;

export function startSSE() {
    if (es) return;
    es = new EventSource('/_/api/stream');

    es.onopen = () => {
        connected = true;
        error = null;
    };

    es.onmessage = (event: MessageEvent) => {
        try {
            data = JSON.parse(event.data) as ConsolePayload;
            lastRefreshed = new Date();
            error = null;
        } catch {
            error = 'Failed to parse server data';
        } finally {
            loading = false;
        }
    };

    es.onerror = () => {
        connected = false;
        loading = false;
        error = 'Connection lost — reconnecting…';
    };
}

export function stopSSE() {
    es?.close();
    es = null;
    connected = false;
}

export function manualRefresh() {
    stopSSE();
    startSSE();
}
