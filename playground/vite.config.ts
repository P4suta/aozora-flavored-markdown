import { defineConfig } from 'vite';
import solid from 'vite-plugin-solid';
import wasm from 'vite-plugin-wasm';

// Served at https://p4suta.github.io/afm/playground/ in production.
// During `vite dev` we mount at root so assets resolve cleanly without
// the path prefix the GitHub Pages deploy demands.
//
// vite-plugin-wasm consumes wasm-pack `--target bundler` output
// (ESM-integrated .wasm with a top-level `await init()`). The companion
// vite-plugin-top-level-await is intentionally absent: per vite-plugin-wasm's
// docs the TLA plugin is only needed for non-`esnext` build targets, and
// `build.target` below is `esnext`, so the top-level await flows through
// natively — the only browsers that run this wasm support it. (vite 8
// dropped the bundled esbuild that the TLA plugin's transform relied on,
// so keeping it would have meant re-adding esbuild for no benefit.)
export default defineConfig(({ command }) => ({
  plugins: [wasm(), solid()],
  base: command === 'build' ? '/afm/playground/' : '/',
  server: {
    host: '0.0.0.0',
    port: 5173,
    strictPort: true,
    fs: {
      // afm-book/theme/*.css lives one level above playground/. Vite's
      // default fs.allow restricts dev-server reads to the project root;
      // widen it so the theme `?url` imports in `src/styles/theme-urls.ts`
      // resolve. Production `build` does not consult this list.
      allow: ['..'],
    },
  },
  preview: {
    host: '0.0.0.0',
    port: 5173,
    strictPort: true,
  },
  build: {
    target: 'esnext',
    sourcemap: true,
    assetsInlineLimit: 0,
    rollupOptions: {
      output: {
        // Split vendor chunks so the initial download budget isn't
        // dominated by a single 800 KB blob that includes CodeMirror,
        // Solid, the lz-string codec, and the app code together.
        // Browsers can request these in parallel, and CodeMirror in
        // particular changes less often than the app code so its
        // chunk stays cached across deploys.
        manualChunks(id) {
          if (
            id.includes('node_modules/@codemirror/') ||
            id.includes('node_modules/@lezer/') ||
            id.includes('node_modules/codemirror/')
          ) {
            return 'vendor-codemirror';
          }
          if (id.includes('node_modules/solid-js/')) {
            return 'vendor-solid';
          }
          if (id.includes('node_modules/lz-string/')) {
            return 'vendor-lz-string';
          }
          // Everything else stays in the main entry chunk; afm-wasm is
          // its own asset via vite-plugin-wasm and not bundled in JS.
          return undefined;
        },
      },
    },
  },
}));
