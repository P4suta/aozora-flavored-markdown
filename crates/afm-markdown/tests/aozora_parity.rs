//! Differential test — `afm_markdown::html` vs `aozora_parser::html`.
//!
//! Both renderers consume the same Aozora-side per-node renderer
//! (`aozora_parser::aozora::html::render`) and the same lexer
//! (`aozora_lexer::lex`). On *pure* aozora input — i.e. text with no
//! CommonMark constructs — the two output paths should produce
//! semantically equivalent HTML, modulo the structural wrapping
//! comrak imposes (`<br />` for hardbreaks, exact whitespace at block
//! seams, etc.).
//!
//! What this differential catches:
//!
//! - **Drift between the two front doors.** A regression that
//!   reaches into `aozora-parser`'s walker and changes the per-node
//!   markup (e.g. flips a class name) immediately breaks both sides
//!   in tandem; one that only edits the afm-side post-process
//!   surfaces here as an asymmetry.
//! - **Class contract leakage.** The set of `afm-*` class tokens in
//!   each output must come from the same pinned source
//!   (`aozora_parser::aozora::AFM_CLASSES`). A renderer that emits
//!   an unregistered class shows up here.
//! - **Tier-A consistency.** Both renderers must satisfy the
//!   no-bare-bracket contract on lexer-clean input.
//!
//! What it does NOT enforce:
//!
//! - Byte-for-byte equality. Comrak's CommonMark renderer wraps text
//!   in slightly different shape (extra `<br />` inside paragraphs,
//!   quoted attribute order). The differential is on
//!   *count and presence* of Aozora-side markers, not exact bytes.

use std::collections::{HashMap, HashSet};

use afm_markdown::html as afm_html;
use aozora_parser::aozora::AFM_CLASSES;
use aozora_parser::html as aozora_html;
use aozora_parser::test_support::{check_no_bare_bracket, check_no_sentinel_leak};

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

/// Tally every `class="afm-*"` token in `html`. Returns a histogram
/// (token → count). Token order doesn't matter for the differential;
/// counts do, because a regression that emits one too many or one
/// too few of a class token usually means a recogniser wired wrong.
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
        let aozora_out = aozora_html::render_to_string(src);
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
            ("aozora", aozora_html::render_to_string(src)),
            ("afm-md", afm_html::render_to_string(src)),
        ] {
            for (token, _count) in afm_class_histogram(&html) {
                if known.contains(token.as_str()) {
                    continue;
                }
                // Numeric-suffix variants like `afm-indent-2`,
                // `afm-container-indent-2`, `afm-align-end-3` are
                // recognised when their stem is in the pinned list.
                if let Some(stem_end) = token.rfind('-') {
                    let stem = &token[..stem_end];
                    let suffix = &token[stem_end + 1..];
                    if known.contains(stem) && suffix.parse::<usize>().is_ok() {
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
    for (label, src) in pure_aozora_fixtures() {
        let aozora_out = aozora_html::render_to_string(src);
        let afm_out = afm_html::render_to_string(src);
        check_no_bare_bracket(&aozora_out)
            .unwrap_or_else(|e| panic!("aozora Tier A on {label} ({src:?}): {e}"));
        check_no_bare_bracket(&afm_out)
            .unwrap_or_else(|e| panic!("afm-markdown Tier A on {label} ({src:?}): {e}"));
    }
}

#[test]
fn both_renderers_satisfy_tier_b_no_pua_leak() {
    for (label, src) in pure_aozora_fixtures() {
        let aozora_out = aozora_html::render_to_string(src);
        let afm_out = afm_html::render_to_string(src);
        check_no_sentinel_leak(&aozora_out)
            .unwrap_or_else(|e| panic!("aozora Tier B on {label} ({src:?}): {e}"));
        check_no_sentinel_leak(&afm_out)
            .unwrap_or_else(|e| panic!("afm-markdown Tier B on {label} ({src:?}): {e}"));
    }
}

#[test]
fn afm_markdown_serialize_delegates_to_aozora_parser() {
    // The afm-markdown serialize path is documented as a
    // 1-line delegate to aozora_parser::serialize_from_artifacts.
    // For any aozora-pipeline parse, the two must agree byte-for-byte.
    use comrak::Arena;
    for (label, src) in pure_aozora_fixtures() {
        // afm-markdown parse + serialize
        let arena = Arena::new();
        let opts = afm_markdown::Options::afm_default();
        let parsed = afm_markdown::parse(&arena, src, &opts);
        let afm_out = afm_markdown::serialize(&parsed);

        // aozora-parser direct
        let aozora_out = aozora_parser::serialize(&aozora_parser::parse(src));

        assert_eq!(
            afm_out, aozora_out,
            "serialize drift on {label} ({src:?}):\n  afm-md: {afm_out:?}\n  aozora: {aozora_out:?}"
        );
    }
}
