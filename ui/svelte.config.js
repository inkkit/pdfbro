// ui/svelte.config.js
import adapter from '@sveltejs/adapter-static';

const config = {
    compilerOptions: {
        runes: ({ filename }) => (filename.split(/[/\\]/).includes('node_modules') ? undefined : true)
    },
    kit: {
        adapter: adapter({ fallback: 'index.html' }),
        paths: { base: '/_' },
    }
};

export default config;
