//! Fuzz target — `aozora_flavored_markdown::html::render_to_string` on arbitrary UTF-8.
//!
//! Arbitrary bytes are decoded as UTF-8 (invalid sequences skip this
//! iteration). The resulting source is pushed through
//! `render_to_string` and every always-on invariant predicate is
//! asserted via [`assert_html_invariants`]. A crash artifact's
//! Debug-formatted panic message is therefore self-contained: tier
//! label + source + html excerpt + violation details — no manual
//! triage needed.
//!
//! Run with:
//! - `just fuzz-quick parse_render` (60 s) — inner-loop smoke
//! - `just fuzz-deep  parse_render` (5 min) — release pre-flight
//! - `just fuzz-triage parse_render`         — replay every artifact
//! - `just fuzz-promote parse_render <hash>` — lift to permanent
//!   regression set under `tests/fuzz_regressions/`

#![no_main]

use aozora_flavored_markdown::html::render_to_string;
use aozora_flavored_markdown_test_support::assert_html_invariants;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(src) = core::str::from_utf8(data) else {
        return;
    };
    let html = render_to_string(src);
    assert_html_invariants(src, &html);
});
