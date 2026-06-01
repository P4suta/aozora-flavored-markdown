//! Snapshot the post-splice HTML for a curated set of afm sources.
//!
//! Sibling to `aozora-render`'s `snapshot_html_golden.rs` — that test
//! pins the *standalone* renderer's `aozora-*` output; this one pins
//! what afm actually ships: comrak-wrapped HTML with the Aozora
//! sentinels spliced in and the brand rewrite (`aozora-` → `afm-`,
//! ADR-0011) applied. A renderer or post-splice regression surfaces
//! here as a one-test `cargo insta` diff instead of an opaque
//! `assert_eq!` mismatch buried in the integration suite.
//!
//! Coverage rationale: each case pins one *kind* of splice path in
//! isolation —
//!
//! * `plain` — the no-sentinel floor (comrak passthrough; the splicer
//!   walks the AST and rewrites nothing).
//! * `inline_ruby` — an inline sentinel spliced inside a `<p>` (the
//!   `INLINE_SENTINEL` → `Raw` inline-node path).
//! * `block_page_break` — a sole-block sentinel promoted to a
//!   block-level `<div class="afm-page-break">` between two paragraphs
//!   (the `BLOCK_LEAF` path + brand rewrite).
//! * `unknown_annotation` — an orphan `［＃…］` the lexer never claimed,
//!   wrapped in a hidden `afm-annotation` span (Tier-A canary: no bare
//!   bracket leaks into body text).
//!
//! Each case passes an explicit snapshot name so the on-disk
//! `.snap` files read as `snapshot_splice__<case>.snap` rather than
//! the default doubled `snapshot_splice__snapshot_<case>.snap` insta
//! would derive from the `snapshot_*` test-function names.
//!
//! Regenerate after an intentional renderer change with
//! `cargo insta accept` (or `INSTA_UPDATE=always cargo test`) inside
//! the dev container — `just test` runs nextest, which does not honour
//! insta's update flow, so accept the pending snapshots with cargo-insta
//! directly.

use afm_markdown::{Options, render_to_string};

/// Render an afm source through the full afm path (lexer pre-pass +
/// comrak + AST splice), returning just the HTML.
fn render(source: &str) -> String {
    render_to_string(source, &Options::afm_default()).html
}

#[test]
fn snapshot_plain() {
    insta::assert_snapshot!("plain", render("hello, world"));
}

#[test]
fn snapshot_inline_ruby() {
    insta::assert_snapshot!("inline_ruby", render("｜青梅《おうめ》へ"));
}

#[test]
fn snapshot_block_page_break() {
    insta::assert_snapshot!("block_page_break", render("前［＃改ページ］後"));
}

#[test]
fn snapshot_unknown_annotation() {
    // Tier-A canary in snapshot form: the bracket text survives only
    // inside the hidden `afm-annotation` wrapper, never bare in body.
    insta::assert_snapshot!("unknown_annotation", render("前［＃ほげふが］後"));
}
