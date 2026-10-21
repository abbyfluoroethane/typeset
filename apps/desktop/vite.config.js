import { defineConfig } from "vite";
import { sveltekit } from "@sveltejs/kit/vite";
import tailwindcss from "@tailwindcss/vite";
import { functionsMixins } from "vite-plugin-functions-mixins";
// @ts-expect-error type error without @types/node package
import process from "node:process";
const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(() => ({
  plugins: [
    // Tailwind's optimizer would hoist m3-svelte's `@apply --mixin` rules out
    // of their selectors before functionsMixins expands them, so leave
    // optimizing to Vite's own minifier, which runs afterwards.
    // Must come before sveltekit(): it registers a Svelte style preprocessor
    // in configResolved, and vite-plugin-svelte snapshots its preprocessors
    // in its own configResolved.
    functionsMixins({ deps: ["m3-svelte"] }),
    tailwindcss({ optimize: false }),
    sveltekit(),
  ],

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  // Pre-bundling compiles m3-svelte's components outside the plugin pipeline,
  // leaving its CSS functions unexpanded.
  optimizeDeps: { exclude: ["m3-svelte"] },

  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || "127.0.0.1",
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
