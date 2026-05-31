// Async entry point for the afm-wasm bundle.
//
// `wasm-pack build --target bundler` ships an ES module that lazily
// instantiates the .wasm on first call. We import everything as a
// namespace so the wasm-bindgen bootstrap runs once at module-eval time.

import * as afmWasm from 'afm-wasm';

// The raw 青空文庫 Document handle + slug catalogue, re-exported for the
// editor-assist layer (completion / hover / inlay / outline / fold /
// linter / structural highlight). These talk to the Aozora parser
// directly — a separate path from `renderAfm` (which goes through comrak
// and loses source offsets). See `crates/afm-wasm/src/lib.rs`.
//
// `Document` is re-exported as both a value (the constructor) and a type
// via this single named re-export — the bundler-target pkg exports it as
// a class. `slugsJson` is wrapped so the panic hook is installed first.
export { Document } from 'afm-wasm';

export function slugsJson(): string {
  ensureInit();
  return afmWasm.slugsJson();
}

// Wire types are generated from the Rust IR + afm-wasm envelope by
// `just types` (xtask) and drift-gated in CI, so the `ir` field below is
// the real `IrDocument` tree rather than `unknown`. Re-exported here so
// existing consumers (diagnostics.ts, App.tsx) keep importing them from
// this module.
import type {
  RenderOptions,
  RenderResult,
} from '../../crates/afm-wasm/types/afm_types';

export type {
  Diagnostic,
  DiagnosticLevel,
  DiagnosticSource,
  IrBlock,
  IrDocument,
  IrInline,
} from '../../crates/afm-wasm/types/afm_types';
export type { RenderOptions, RenderResult };

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
