// Async entry point for the afm-wasm bundle.
//
// `wasm-pack build --target bundler` ships an ES module that lazily
// instantiates the .wasm on first call. We import everything as a
// namespace so the wasm-bindgen bootstrap runs once at module-eval time.

import * as afmWasm from 'afm-wasm';

export type DiagnosticLevel = 'error' | 'warning' | 'note';
export type DiagnosticSource = 'source' | 'internal';

export interface Diagnostic {
  readonly level: DiagnosticLevel;
  readonly source: DiagnosticSource;
  readonly code: string;
  readonly message: string;
}

export interface RenderResult {
  readonly html: string;
  readonly diagnostics: readonly Diagnostic[];
  // `ir` is the structured projection. Unused by the playground UI today
  // but typed loosely so downstream code can opt in without a recompile.
  readonly ir: unknown;
}

export interface RenderOptions {
  readonly aozoraEnabled?: boolean;
  readonly sourceLineAnchors?: boolean;
}

let initialised = false;

// Block the UI thread on first render until the wasm has booted.
// `--target bundler` arranges synchronous lazy init the first time an
// export is touched, but we still call `initPanicHook` so any panic
// inside the renderer lands in the browser console with a readable
// trace instead of "unreachable executed".
function ensureInit(): void {
  if (initialised) return;
  afmWasm.initPanicHook();
  initialised = true;
}

export function renderAfm(source: string, options?: RenderOptions): RenderResult {
  ensureInit();
  return afmWasm.renderAfm(source, options as unknown) as RenderResult;
}

export function hashSource(source: string): bigint {
  ensureInit();
  return afmWasm.hashSource(source);
}
