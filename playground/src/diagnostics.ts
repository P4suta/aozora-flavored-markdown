// Re-exports for `Diagnostic` consumers. The Solid component lives in
// `components/DiagnosticsDrawer.tsx`; this module exists so callers can
// `import type { Diagnostic } from './diagnostics'` without reaching
// into the wasm bridge module name.

export type { Diagnostic, DiagnosticLevel, DiagnosticSource } from './wasm-loader';
