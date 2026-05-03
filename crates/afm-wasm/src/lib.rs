//! WebAssembly bindings for afm-markdown.
//!
//! Exposes a thin set of `#[wasm_bindgen]` exports that
//! afm-obsidian (and other browser hosts) call across the WASM
//! boundary. The IR shape returned by `render_afm` and
//! `render_aozora_only` mirrors the TS `IRDocument` defined in
//! `afm-obsidian/src/ir/types.ts` and is validated on the JS side
//! by `from-wasm.ts`.
//!
//! # Stability
//!
//! Public exports here are version-pinned to afm-markdown's
//! workspace version. A bump on this crate implies an afm-obsidian
//! recompilation against the new IR shape.
//!
//! # Surface
//!
//! - [`init_panic_hook`] — opt-in panic forwarder (debug builds).
//! - [`render_afm`] — full afm pipeline (CommonMark + GFM + aozora).
//! - [`render_aozora_only`] — aozora-only inline mode (used by
//!   afm-obsidian's inline post-processor; bypasses comrak).
//! - [`hash_source`] — xxh3-64 over the source, returned as `u64`
//!   for cache-key construction on the JS side.

#![forbid(unsafe_code)]

use afm_markdown::ir::{IrBlock, IrDocument};
use afm_markdown::{Options, render_blocks_to_ir, render_to_ir};
use serde::Serialize;
use twox_hash::XxHash3_64;
use wasm_bindgen::prelude::*;

/// Install a `console.error` panic hook for friendlier debugging.
/// No-op when compiled without the `panic-hook` feature.
#[wasm_bindgen(js_name = initPanicHook)]
pub fn init_panic_hook() {
    #[cfg(feature = "panic-hook")]
    {
        console_error_panic_hook::set_once();
    }
}

/// Result envelope returned to JS. Matches the shape consumed by
/// `afm-obsidian/src/ir/from-wasm.ts`.
#[derive(Serialize)]
struct RenderResult {
    /// Structured IR — see `afm_markdown::ir` for the type tree.
    /// Mirrors the TS `IRDocument` (camelCase fields, discriminated
    /// unions on `kind`).
    ir: IrDocument,
    /// Reference HTML (post-aozora-splice + source-line anchored).
    /// Consumers may render straight from the IR via the JS
    /// renderers; this string is a debug / fallback surface and a
    /// lifeline for hosts that don't ship a JS renderer.
    html: String,
    diagnostics: Vec<DiagnosticOut>,
}

#[derive(Serialize)]
struct DiagnosticOut {
    level: &'static str,
    message: String,
}

/// Optional render configuration accepted from JS. All fields are
/// optional so callers can omit them entirely (legacy callers pass
/// `undefined`).
#[derive(serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct RenderOptions {
    aozora_enabled: Option<bool>,
    source_line_anchors: Option<bool>,
}

fn build_options(opts: &RenderOptions) -> Options<'static> {
    let mut base = Options::afm_default();
    if let Some(v) = opts.aozora_enabled {
        base.aozora_enabled = v;
    }
    if let Some(v) = opts.source_line_anchors {
        base.source_line_anchors = v;
    }
    base
}

/// Render afm source to IR + HTML + diagnostics.
///
/// `options` is decoded as `{ aozoraEnabled?: boolean,
/// sourceLineAnchors?: boolean }`. Both default to the values from
/// `Options::afm_default()` (aozora on, anchors off).
///
/// # Errors
///
/// Returns `Err(JsValue::String)` when `options` cannot be deserialized
/// from JS or when the resulting `RenderResult` cannot be serialized
/// back to JS.
#[wasm_bindgen(js_name = renderAfm)]
pub fn render_afm(source: &str, options: JsValue) -> Result<JsValue, JsValue> {
    let opts: RenderOptions = if options.is_undefined() || options.is_null() {
        RenderOptions::default()
    } else {
        serde_wasm_bindgen::from_value(options).map_err(|e| JsValue::from_str(&e.to_string()))?
    };
    let resolved = build_options(&opts);
    let rendered = render_to_ir(source, &resolved);
    let result = RenderResult {
        ir: rendered.ir,
        html: rendered.html,
        diagnostics: rendered
            .diagnostics
            .iter()
            .map(|d| DiagnosticOut {
                level: "info",
                message: format!("{d:?}"),
            })
            .collect(),
    };
    serde_wasm_bindgen::to_value(&result).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Render aozora-only inline text (no markdown re-parse).
///
/// For v0.1 this routes through the same `render_to_ir`; once
/// `aozora-render` exposes a standalone "render this text as
/// inline aozora HTML" entry point we'll switch to it for
/// performance.
///
/// # Errors
///
/// Returns `Err(JsValue::String)` when the resulting `RenderResult`
/// cannot be serialized back to JS.
#[wasm_bindgen(js_name = renderAozoraOnly)]
pub fn render_aozora_only(text: &str) -> Result<JsValue, JsValue> {
    render_afm(text, JsValue::UNDEFINED)
}

/// xxh3-64 over the source, returned as a `u64` (JS receives a
/// `bigint`). Used for cache keys.
#[must_use]
#[wasm_bindgen(js_name = hashSource)]
pub fn hash_source(source: &str) -> u64 {
    XxHash3_64::oneshot_with_seed(0, source.as_bytes())
}

#[derive(Serialize)]
struct BlockResult {
    ir: IrBlock,
    html: String,
    /// 1-based source line.
    source_line: u32,
}

#[derive(Serialize)]
struct BlocksResult {
    blocks: Vec<BlockResult>,
    diagnostics: Vec<DiagnosticOut>,
}

/// Per-block streaming render.
///
/// Returns one `{ir, html, sourceLine}` entry per top-level comrak
/// block. The afm-obsidian bridge iterates the array and checks its
/// `AbortSignal` between blocks (ADR-0009 chunked-cancellation
/// strategy).
///
/// # Errors
///
/// Returns `Err(JsValue::String)` when `options` cannot be deserialized
/// from JS or when the resulting `BlocksResult` cannot be serialized
/// back to JS.
#[wasm_bindgen(js_name = renderBlocks)]
pub fn render_blocks(source: &str, options: JsValue) -> Result<JsValue, JsValue> {
    let opts: RenderOptions = if options.is_undefined() || options.is_null() {
        RenderOptions::default()
    } else {
        serde_wasm_bindgen::from_value(options).map_err(|e| JsValue::from_str(&e.to_string()))?
    };
    let resolved = build_options(&opts);
    let (blocks, diagnostics) = render_blocks_to_ir(source, &resolved);
    let result = BlocksResult {
        blocks: blocks
            .into_iter()
            .map(|b| BlockResult {
                ir: b.ir,
                html: b.html,
                source_line: b.source_line,
            })
            .collect(),
        diagnostics: diagnostics
            .iter()
            .map(|d| DiagnosticOut {
                level: "info",
                message: format!("{d:?}"),
            })
            .collect(),
    };
    serde_wasm_bindgen::to_value(&result).map_err(|e| JsValue::from_str(&e.to_string()))
}
