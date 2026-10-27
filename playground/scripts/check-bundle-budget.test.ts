import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { gzipSync } from 'node:zlib';
import { afterEach, expect, it } from 'vitest';

const script = resolve('scripts/check-bundle-budget.ts');
const directories: string[] = [];
const reference = '/aozora-flavored-markdown/playground/assets/nested/entry.js';
const entry = `export const payload = "${Array.from(
  { length: 4096 },
  (_, index) => createHash('sha256').update(String(index)).digest('hex'),
).join('')}";`;

async function bundle(html: string, contents = entry): Promise<string> {
  const directory = await mkdtemp(join(tmpdir(), 'afm-bundle-'));
  directories.push(directory);
  const assets = join(directory, 'dist/assets');
  await mkdir(join(assets, 'nested'), { recursive: true });
  await writeFile(join(directory, 'dist/index.html'), html);
  await writeFile(join(assets, 'nested/entry.js'), contents);
  for (const name of [
    'vendor-codemirror.js',
    'adapter-engine.js',
    'engineFeatures.js',
    'wasm-loader.js',
    'engine.wasm',
  ]) {
    await writeFile(join(assets, name), 'deferred');
  }
  return directory;
}

afterEach(async () => {
  await Promise.all(
    directories
      .splice(0)
      .map((directory) => rm(directory, { recursive: true })),
  );
});

it('counts a prefixed nested asset once across module and preload references', async () => {
  const directory = await bundle(
    `<script type="module" src="${reference}"></script>` +
      `<link rel="modulepreload" href="${reference}">` +
      `<script type="module" src="${reference}"></script>`,
  );
  const bytes = gzipSync(entry).byteLength;
  expect(bytes).toBeGreaterThan(105 * 1024);
  expect(bytes).toBeLessThan(210 * 1024);
  const result = spawnSync('bun', [script], {
    cwd: directory,
    encoding: 'utf8',
  });
  expect(result.error).toBeUndefined();
  expect(result.status, result.stderr).toBe(0);
  const reported = /^initial \.js: (\d+\.\d) KiB \/ 210 KiB$/m.exec(
    result.stdout,
  );
  expect(reported).not.toBeNull();
  expect(Number(reported?.[1])).toBeCloseTo(bytes / 1024, 0);
});

it.each([
  '<script type="module" src="/assets/missing.js"></script>',
  '<link rel="modulepreload" href="/assets/missing.js">',
])('rejects a missing initial asset in %s', async (html) => {
  const directory = await bundle(html);
  const result = spawnSync('bun', [script], {
    cwd: directory,
    encoding: 'utf8',
  });
  expect(result.error).toBeUndefined();
  expect(result.status).toBe(1);
  expect(result.stderr).toContain(
    'initial JavaScript asset is missing: /assets/missing.js',
  );
});

it('rejects initial JavaScript that exceeds its compressed size limit', async () => {
  const contents = `${entry}${entry.replaceAll('payload', 'secondPayload')}`;
  const directory = await bundle(
    `<script type="module" src="${reference}"></script>`,
    contents,
  );
  const bytes = gzipSync(contents).byteLength;
  expect(bytes).toBeGreaterThan(210 * 1024);
  expect(bytes).toBeLessThan(440 * 1024);
  const result = spawnSync('bun', [script], {
    cwd: directory,
    encoding: 'utf8',
  });
  expect(result.error).toBeUndefined();
  expect(result.status).toBe(1);
  expect(result.stderr).not.toContain('initial JavaScript asset is missing');
  const reported = /^initial \.js: (\d+\.\d) KiB \/ 210 KiB$/m.exec(
    result.stderr,
  );
  expect(reported).not.toBeNull();
  expect(Number(reported?.[1])).toBeCloseTo(bytes / 1024, 0);
});
