//! Native (non-wasm) smoke tests for the afm-wasm crate.
//!
//! `cargo test -p afm-wasm` builds the crate as a regular `rlib`
//! (the `[lib].crate-type` includes `rlib` for exactly this reason)
//! so we can validate the underlying logic without spinning up a
//! browser / Node WASM runtime.
//!
//! These tests do NOT exercise the wasm-bindgen marshalling path —
//! that's covered by afm-obsidian's `from-wasm.test.ts` against a
//! built `.wasm` artefact.

use afm_wasm::hash_source;

#[test]
fn hash_source_is_deterministic() {
    assert_eq!(hash_source("hello"), hash_source("hello"));
}

#[test]
fn hash_source_differs_for_different_inputs() {
    assert_ne!(hash_source("hello"), hash_source("world"));
}

#[test]
fn hash_source_is_nonzero_for_typical_input() {
    assert_ne!(hash_source(""), hash_source("｜漢字《かんじ》"));
}
