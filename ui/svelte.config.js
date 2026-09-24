// Shared by vite-plugin-svelte, svelte-check, eslint-plugin-svelte and prettier-plugin-svelte.
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

/** @type {import('@sveltejs/vite-plugin-svelte').SvelteConfig} */
export default {
  preprocess: vitePreprocess(),
  compilerOptions: {
    runes: true,
  },
};
