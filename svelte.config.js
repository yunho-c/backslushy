// Tauri doesn't have a Node.js server to do proper SSR
// so we use adapter-static with a fallback to index.html to put the site in SPA mode
// See: https://svelte.dev/docs/kit/single-page-apps
// See: https://v2.tauri.app/start/frontend/sveltekit/ for more info
import adapter from "@sveltejs/adapter-static";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

/** @type {import('@sveltejs/kit').Config} */
const config = {
  preprocess: vitePreprocess(),
  kit: {
    alias: {
      $lib: "src/lib",
      "$lib/*": "src/lib/*",
      "$lib/components": "src/lib/components",
      "$lib/components/*": "src/lib/components/*",
      "$lib/utils": "src/lib/utils",
      "$lib/hooks": "src/lib/hooks",
      "$lib/hooks/*": "src/lib/hooks/*",
    },
    adapter: adapter({
      fallback: "index.html",
    }),
  },
};

export default config;
