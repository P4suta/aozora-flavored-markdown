import { defineConfig } from 'vite';
import solid from 'vite-plugin-solid';
import topLevelAwait from 'vite-plugin-top-level-await';
import wasm from 'vite-plugin-wasm';

// Served at https://p4suta.github.io/afm/playground/ in production.
// During `vite dev` we mount at root so assets resolve cleanly without
// the path prefix the GitHub Pages deploy demands.
//
// wasm + topLevelAwait are required to consume wasm-pack `--target
// bundler` output (ESM-integrated .wasm with top-level `await init()`).
export default defineConfig(({ command }) => ({
  plugins: [wasm(), topLevelAwait(), solid()],
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
    target: 'es2022',
    sourcemap: true,
    assetsInlineLimit: 0,
  },
}));
