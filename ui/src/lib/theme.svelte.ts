// src/lib/theme.svelte.ts
export let dark = $state(false);
export let accent = $state('#4f8ef7');
export let density = $state<'compact' | 'regular' | 'comfy'>('regular');

export type ThemeTokens = {
    bg: string; surface: string; ink: string; muted: string;
    faint: string; rule: string; ok: string; warn: string; err: string; accent: string;
};

export let theme = $derived<ThemeTokens>({
    bg:      dark ? '#0e0f12' : '#f7f7f5',
    surface: dark ? '#15171c' : '#ffffff',
    ink:     dark ? '#e6e7ea' : '#1a1c1f',
    muted:   dark ? 'rgba(230,231,234,0.55)' : 'rgba(26,28,31,0.55)',
    faint:   dark ? 'rgba(230,231,234,0.10)' : 'rgba(26,28,31,0.06)',
    rule:    dark ? 'rgba(255,255,255,0.08)' : 'rgba(26,28,31,0.08)',
    ok:      dark ? '#3fb27f' : '#2f9967',
    warn:    dark ? '#e0a93c' : '#b8860b',
    err:     dark ? '#e26464' : '#c25151',
    accent,
});

export let D = $derived(
    density === 'compact'
        ? { gap: 8,  pad: 8,  rowPy: 2, fz: 10.5, kpiFz: 18 }
        : density === 'comfy'
            ? { gap: 14, pad: 14, rowPy: 5, fz: 12,   kpiFz: 22 }
            : { gap: 10, pad: 10, rowPy: 3, fz: 11.5, kpiFz: 20 }
);

// Persist dark mode in localStorage (client-side only)
if (typeof window !== 'undefined') {
    dark = localStorage.getItem('folio-dark') === 'true';
}

$effect.root(() => {
    $effect(() => {
        if (typeof window !== 'undefined') {
            localStorage.setItem('folio-dark', String(dark));
            document.documentElement.classList.toggle('dark', dark);
        }
    });
});
