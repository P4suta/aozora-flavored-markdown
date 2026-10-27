import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { createServer, normalizePath } from 'vite';
import wasm from 'vite-plugin-wasm';
import { afterEach, expect, it, vi } from 'vitest';

interface RuntimeCase {
  name: string;
  flag: 'true' | 'false' | undefined;
  pluginName: string;
  embedded: boolean;
}

const runtimes: readonly RuntimeCase[] = [
  { name: 'Vitest 4', flag: undefined, pluginName: 'vitest', embedded: true },
  {
    name: 'Vitest 5',
    flag: 'true',
    pluginName: 'vitest:config:append',
    embedded: true,
  },
  {
    name: 'a renamed Vitest plugin',
    flag: 'true',
    pluginName: 'test-runner',
    embedded: true,
  },
  { name: 'Vite', flag: undefined, pluginName: 'vite', embedded: false },
  {
    name: 'an unrelated plugin',
    flag: undefined,
    pluginName: 'vitest-helper',
    embedded: false,
  },
  {
    name: 'an explicitly disabled flag',
    flag: 'false',
    pluginName: 'vite',
    embedded: false,
  },
];

afterEach(() => {
  vi.unstubAllEnvs();
});

it.each(runtimes)(
  'preserves the WebAssembly loading contract for $name',
  async ({ flag, pluginName, embedded }) => {
    vi.stubEnv('VITEST', flag);
    const server = await createServer({
      configFile: false,
      plugins: [wasm(), { name: pluginName }],
      server: { middlewareMode: true, watch: null },
      optimizeDeps: { noDiscovery: true },
    });
    try {
      const path = normalizePath(
        resolve(
          '../crates/aozora-flavored-markdown-wasm/pkg/aozora_flavored_markdown_wasm_bg.wasm',
        ),
      );
      const loaded =
        await server.environments.client.pluginContainer.load(path);
      expect(loaded).not.toBeNull();
      const code = typeof loaded === 'string' ? loaded : loaded?.code;
      if (embedded) {
        const bytes = await readFile(path);
        expect(
          code?.includes(
            `data:application/wasm;base64,${bytes.toString('base64')}`,
          ),
        ).toBe(true);
      } else {
        expect(code?.includes(`${path}?url`)).toBe(true);
        expect(code?.includes('data:application/wasm;base64,')).toBe(false);
      }
    } finally {
      await server.close();
    }
  },
);
