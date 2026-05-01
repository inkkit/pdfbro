// src/lib/metrics.svelte.ts
import type { ConsolePayload } from './types';

class MetricsStore {
    data         = $state<ConsolePayload | null>(null);
    loading      = $state(true);
    connected    = $state(false);
    error        = $state<string | null>(null);
    lastRefreshed = $state<Date | null>(null);

    #es: EventSource | null = null;

    start() {
        if (this.#es) return;
        this.#es = new EventSource('/_/api/stream');

        this.#es.onopen = () => {
            this.connected = true;
            this.error = null;
        };

        this.#es.onmessage = (event: MessageEvent) => {
            try {
                this.data = JSON.parse(event.data) as ConsolePayload;
                this.lastRefreshed = new Date();
                this.error = null;
            } catch {
                this.error = 'Failed to parse server data';
            } finally {
                this.loading = false;
            }
        };

        this.#es.onerror = () => {
            this.connected = false;
            this.loading = false;
            this.error = 'Connection lost — reconnecting…';
        };
    }

    stop() {
        this.#es?.close();
        this.#es = null;
        this.connected = false;
    }

    refresh() {
        this.stop();
        this.start();
    }
}

export const metricsStore = new MetricsStore();
