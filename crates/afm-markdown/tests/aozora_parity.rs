//! Differential test — `afm_markdown` vs `aozora-render`.
//!
//! Both pipelines consume the same lexer output (`aozora_lex::lex_into_arena`)
//! and the same per-node renderer (`aozora_render::render_node`), so on
//! pure-青空文庫 input the *count and presence* of every `afm-*` class
//! token must match. Block structure differs (afm-markdown wraps paragraphs
//! through comrak; aozora-render emits its own `<p>` tags), so the
//! differential is on histograms, not on byte equality.
//!
//! What this differential catches:
//!
//! - **Drift between the two front doors.** A regression that flips a
//!   class name in `aozora-render` breaks both sides in tandem; one
//!   that only edits the afm-side post-process surfaces as an
//!   asymmetry.
//! - **Class contract leakage.** Both renderers source their `afm-*`
//!   classes from the same pinned list (`AFM_CLASSES` in
//!   `afm_markdown::test_support`). A renderer that emits an
//!   unregistered class shows up here.
//! - **Tier-A / Tier-B consistency.** Both renderers must satisfy the
//!   no-bare-bracket and no-PUA-leak contracts on lexer-clean input.
//! - **Serializer equivalence.** `afm_markdown::serialize` is a thin
//!   delegate to `aozora_render::serialize::serialize`; on the same
//!   source they must produce identical bytes.

use std::collections::{HashMap, HashSet};

use afm_markdown::html as afm_html;
use afm_markdown::test_support::AFM_CLASSES;
use aozora_lex::lex_into_arena;
use aozora_render::html as aozora_html;
use aozora_render::serialize as aozora_serialize;
use aozora_syntax::borrowed::Arena;

/// A handful of pure-aozora source fragments — no CommonMark
/// emphasis, no headings, no lists, no code spans. Each fragment
/// exercises a distinct aozora-side recogniser so the differential
/// has signal even on a small corpus.
fn pure_aozora_fixtures() -> &'static [(&'static str, &'static str)] {
    &[
        ("plain ASCII", "Hello, world."),
        ("plain Japanese", "親譲りの無鉄砲"),
        ("explicit ruby", "｜青梅《おうめ》"),
        ("implicit ruby", "親譲《おやゆず》り"),
        ("forward bouten", "可哀想［＃「可哀想」に傍点］"),
        ("page break standalone", "［＃改ページ］"),
        ("page break mid", "前［＃改ページ］後"),
        ("section break choho", "［＃改丁］"),
        ("indent leaf", "［＃地付き］"),
        (
            "two ruby in one paragraph",
            "｜青梅《おうめ》と｜鶴見《つるみ》の間",
        ),
        (
            "indent container",
            "［＃ここから2字下げ］\n\n本文\n\n［＃ここで字下げ終わり］",
        ),
        ("multi paragraph", "first\n\nsecond"),
    ]
}

/// Render `src` through `aozora-render::html` (pure-aozora pipeline).
fn aozora_only_render(src: &str) -> String {
    let arena = Arena::new();
    let lex_out = lex_into_arena(src, &arena);
    aozora_html::render_to_string(&lex_out)
}

/// Tally every `class="afm-*"` token in `html`. Returns a histogram
/// (token → count). Token order doesn't matter for the differential;
/// counts do.
fn afm_class_histogram(html: &str) -> HashMap<String, usize> {
    let mut hist = HashMap::new();
    for token_run in html.split("class=\"").skip(1) {
        let Some(end) = token_run.find('"') else {
            continue;
        };
        for token in token_run[..end].split_ascii_whitespace() {
            if token.starts_with("afm-") {
                *hist.entry(token.to_owned()).or_insert(0) += 1;
            }
        }
    }
    hist
}

#[test]
fn both_renderers_agree_on_afm_class_histogram_for_pure_aozora_input() {
    let mut diffs = Vec::new();
    for (label, src) in pure_aozora_fixtures() {
        let aozora_out = aozora_only_render(src);
        let afm_out = afm_html::render_to_string(src);
        let aozora_hist = afm_class_histogram(&aozora_out);
        let afm_hist = afm_class_histogram(&afm_out);
        if aozora_hist != afm_hist {
            diffs.push(format!(
                "{label} ({src:?})\n  aozora: {aozora_hist:?}\n  afm:    {afm_hist:?}"
            ));
        }
    }
    assert!(
        diffs.is_empty(),
        "afm-* class histograms diverge between renderers:\n\n{}",
        diffs.join("\n\n"),
    );
}

#[test]
fn every_afm_class_emitted_is_in_the_pinned_contract() {
    // Both renderers must source their afm-* classes from the same
    // pinned list. A regression that emitted, say, `afm-bouten-foo`
    // for a previously-unknown bouten kind would surface here.
    let known: HashSet<&'static str> = AFM_CLASSES.iter().copied().collect();
    let mut violations = Vec::new();
    for (label, src) in pure_aozora_fixtures() {
        for (renderer, html) in [
            ("aozora", aozora_only_render(src)),
            ("afm-md", afm_html::render_to_string(src)),
        ] {
            for (token, _count) in afm_class_histogram(&html) {
                if known.contains(token.as_str()) {
                    continue;
                }
                // Family-suffix variants — `afm-indent-2`,
                // `afm-section-break-choho`, `afm-bouten-goma`-suffixed
                // forms, etc. The pinned list carries the family stem;
                // we accept any suffix as long as the stem is known.
                if let Some(stem_end) = token.rfind('-') {
                    let stem = &token[..stem_end];
                    if known.contains(stem) {
                        continue;
                    }
                }
                violations.push(format!(
                    "{renderer} emitted unknown afm-* class {token:?} for {label} ({src:?})"
                ));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "unknown afm-* classes:\n  {}",
        violations.join("\n  "),
    );
}

#[test]
fn both_renderers_satisfy_tier_a_no_bare_bracket() {
    use afm_markdown::test_support::strip_annotation_wrappers;
    for (label, src) in pure_aozora_fixtures() {
        for (renderer, html) in [
            ("aozora", aozora_only_render(src)),
            ("afm-md", afm_html::render_to_string(src)),
        ] {
            let stripped = strip_annotation_wrappers(&html);
            assert!(
                !stripped.contains("［＃"),
                "{renderer} Tier A leaked ［＃ on {label} ({src:?}): {html}"
            );
        }
    }
}

#[test]
fn both_renderers_satisfy_tier_b_no_pua_leak() {
    use afm_markdown::{
        BLOCK_CLOSE_SENTINEL, BLOCK_LEAF_SENTINEL, BLOCK_OPEN_SENTINEL, INLINE_SENTINEL,
    };
    for (label, src) in pure_aozora_fixtures() {
        for (renderer, html) in [
            ("aozora", aozora_only_render(src)),
            ("afm-md", afm_html::render_to_string(src)),
        ] {
            for s in [
                INLINE_SENTINEL,
                BLOCK_LEAF_SENTINEL,
                BLOCK_OPEN_SENTINEL,
                BLOCK_CLOSE_SENTINEL,
            ] {
                assert!(
                    !html.contains(s),
                    "{renderer} Tier B leaked sentinel {s:?} on {label} ({src:?}): {html}"
                );
            }
        }
    }
}

#[test]
fn afm_markdown_serialize_matches_aozora_render_serialize() {
    // afm_markdown::serialize is a thin delegate to
    // aozora_render::serialize::serialize, so the two must produce
    // identical bytes for the same source.
    for (label, src) in pure_aozora_fixtures() {
        let arena = Arena::new();
        let lex_out = lex_into_arena(src, &arena);
        let aozora_out = aozora_serialize::serialize(&lex_out);
        let afm_out = afm_markdown::serialize(src);
        assert_eq!(
            afm_out, aozora_out,
            "serialize drift on {label} ({src:?}):\n  afm-md: {afm_out:?}\n  aozora: {aozora_out:?}"
        );
    }
}
