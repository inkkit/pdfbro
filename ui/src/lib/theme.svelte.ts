// src/lib/theme.svelte.ts

export type ThemeTokens = {
    bg: string; surface: string; ink: string; muted: string;
    faint: string; rule: string; ok: string; warn: string; err: string; accent: string;
};
export type Theme = ThemeTokens;

class ThemeStore {
    dark  = $state(false);
    accent = $state('#4f8ef7');
    density = $state<'compact' | 'regular' | 'comfy'>('regular');

    get theme(): ThemeTokens {
        return {
            bg:      this.dark ? '#0e0f12' : '#f7f7f5',
            surface: this.dark ? '#15171c' : '#ffffff',
            ink:     this.dark ? '#e6e7ea' : '#1a1c1f',
            muted:   this.dark ? 'rgba(230,231,234,0.55)' : 'rgba(26,28,31,0.55)',
            faint:   this.dark ? 'rgba(230,231,234,0.10)' : 'rgba(26,28,31,0.06)',
            rule:    this.dark ? 'rgba(255,255,255,0.08)' : 'rgba(26,28,31,0.08)',
            ok:      this.dark ? '#3fb27f' : '#2f9967',
            warn:    this.dark ? '#e0a93c' : '#b8860b',
            err:     this.dark ? '#e26464' : '#c25151',
            accent:  this.accent,
        };
    }

    get D() {
        return this.density === 'compact'
            ? { gap: 8,  pad: 8,  rowPy: 2, fz: 10.5, kpiFz: 18 }
            : this.density === 'comfy'
                ? { gap: 14, pad: 14, rowPy: 5, fz: 12,   kpiFz: 22 }
                : { gap: 10, pad: 10, rowPy: 3, fz: 11.5, kpiFz: 20 };
    }
}

export const themeStore = new ThemeStore();

// Persist dark mode in localStorage (client-side only)
if (typeof window !== 'undefined') {
    themeStore.dark = localStorage.getItem('folio-dark') === 'true';
}

$effect.root(() => {
    $effect(() => {
        if (typeof window !== 'undefined') {
            localStorage.setItem('folio-dark', String(themeStore.dark));
            document.documentElement.classList.toggle('dark', themeStore.dark);
        }
    });
});
