//! Class-contract test between renderer and themes.
//!
//! The afm-book ships two CSS themes (`afm-horizontal.css` and
//! `afm-vertical.css`) whose class selectors must cover every
//! class token the renderer can emit. Without this contract a
//! renderer change (e.g. a new `afm-bouten-foo` kind) silently
//! ships unstyled markup — invisible in unit tests because the
//! existing HTML assertions don't care about CSS.
//!
//! The test reads both CSS files at runtime, extracts every
//! `.afm-*` selector, and asserts each pinned class token appears
//! in both. If a renderer adds a class, the pinned list in this
//! file flags the missing style at `cargo test` time.
//!
//! The pinned list is intentionally hand-curated (not derived by
//! scraping `html.rs`) so that adding a new class forces an
//! *explicit* update here — drive-by CSS gaps can't slip in.
//!
//! Those theme tests prove `AFM_CLASSES ⊆ CSS`. The render-direction
//! tests at the bottom of this file close the other half of the loop
//! (`construct → afm-* class → AFM_CLASSES`): they render a curated
//! corpus of every aozora construct afm knows and assert both that no
//! emitted class is missing from `AFM_CLASSES` and that no `AFM_CLASSES`
//! entry has gone stale. This is the pin-independent half of #74.

use core::str;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use afm_markdown::html::render_to_string;
use afm_markdown_test_support::{AFM_CLASSES, check_css_class_contract};

/// Absolute path to one of the theme CSS files. Resolving via
/// `CARGO_MANIFEST_DIR` keeps the test stable regardless of the
/// runner's working directory.
fn theme_path(name: &str) -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop(); // crates/afm-markdown → crates/
    p.push("afm-book");
    p.push("theme");
    p.push(name);
    p
}

/// Return every `.afm-…` class selector name appearing in `css`.
///
/// A tokeniser that extracts the identifier immediately after each
/// `.afm-` prefix. Accepts lowercase ASCII letters, digits, and
/// hyphens; stops at any other character. Intentionally trivial —
/// the project's CSS doesn't use namespace prefixes or escaped
/// selectors.
fn collect_afm_selectors(css: &str) -> HashSet<String> {
    let mut out = HashSet::new();
    let bytes = css.as_bytes();
    let mut i = 0usize;
    while i + 5 <= bytes.len() {
        if &bytes[i..i + 5] == b".afm-" {
            let start = i + 1; // after the '.'
            let mut end = start;
            while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'-') {
                end += 1;
            }
            // Trim trailing hyphens from a ".afm-" prefix with no
            // body — shouldn't occur but be tolerant.
            let token = str::from_utf8(&bytes[start..end]).expect("ASCII");
            if token.len() > "afm-".len() && !token.ends_with('-') {
                out.insert(token.to_owned());
            }
            i = end;
        } else {
            i += 1;
        }
    }
    out
}

#[test]
fn every_emitted_class_has_a_horizontal_theme_rule() {
    let css = fs::read_to_string(theme_path("afm-horizontal.css"))
        .expect("afm-horizontal.css must exist alongside afm-book/theme/");
    let selectors = collect_afm_selectors(&css);
    let missing: Vec<&&str> = AFM_CLASSES
        .iter()
        .filter(|c| !selectors.contains(**c))
        .collect();
    assert!(
        missing.is_empty(),
        "afm-horizontal.css is missing rules for emitted classes: {missing:?}"
    );
}

#[test]
fn every_emitted_class_has_a_vertical_theme_rule() {
    let css = fs::read_to_string(theme_path("afm-vertical.css"))
        .expect("afm-vertical.css must exist alongside afm-book/theme/");
    let selectors = collect_afm_selectors(&css);
    let missing: Vec<&&str> = AFM_CLASSES
        .iter()
        .filter(|c| !selectors.contains(**c))
        .collect();
    assert!(
        missing.is_empty(),
        "afm-vertical.css is missing rules for emitted classes: {missing:?}"
    );
}

#[test]
fn pinned_classes_are_sorted_and_unique() {
    // Hygiene: the pinned list is kept in sorted order + no dupes
    // for review friendliness. A sorted list also makes PR diffs
    // trivial to review.
    let mut copy: Vec<&str> = AFM_CLASSES.to_vec();
    copy.sort_unstable();
    assert_eq!(AFM_CLASSES.to_vec(), copy, "AFM_CLASSES must stay sorted");
    let mut seen: HashSet<&str> = HashSet::new();
    for &c in AFM_CLASSES {
        assert!(seen.insert(c), "duplicate entry in AFM_CLASSES: {c}");
    }
}

#[test]
fn collect_afm_selectors_extracts_basic_rules() {
    // Self-test for the tokeniser — a regression here would
    // silently weaken every other test in this file.
    let css = ".afm-foo { color: red; }\n.afm-bar-baz, .afm-qux { }\n.foo { }";
    let selectors = collect_afm_selectors(css);
    assert!(selectors.contains("afm-foo"));
    assert!(selectors.contains("afm-bar-baz"));
    assert!(selectors.contains("afm-qux"));
    assert!(!selectors.contains("foo"));
}

#[test]
fn collect_afm_selectors_tolerates_trailing_hyphen() {
    // `.afm-` alone (no body) must not emit a token.
    let css = ".afm- { }";
    let selectors = collect_afm_selectors(css);
    assert!(selectors.is_empty());
}

// ---------------------------------------------------------------------------
// Render-direction contract: construct → afm-* class → AFM_CLASSES
//
// `every_emitted_class_has_a_*_theme_rule` proves AFM_CLASSES ⊆ CSS. The
// two tests below close the other half of the loop:
//
//   (1) every afm-* class a known construct emits is recognised by
//       AFM_CLASSES — a *new* upstream class surfaces here by name the
//       moment the corpus reaches it (pin bump), instead of shipping
//       silent unstyled markup; and
//   (2) every AFM_CLASSES entry is actually emitted by some construct —
//       a *stale* entry (e.g. `afm-double-ruby` after the upstream
//       AngleQuote rename) surfaces here — modulo the documented
//       UNEXERCISED gaps.
//
// This is the pin-independent half of #74. It does not eliminate the
// manual AFM_CLASSES append (that is the deferred build.rs source-scrape);
// it turns drift into a loud, named failure. Every source below is copied
// verbatim from an existing passing afm test (paired_container.rs /
// html_well_formed.rs / ir_aozora.rs) or from aozora-render's own tests at
// the pinned SHA — none authored from memory.
// ---------------------------------------------------------------------------

/// One verified afm source per class-emitting aozora construct.
const RENDER_CORPUS: &[(&str, &str)] = &[
    ("ruby (explicit)", "｜青梅《おうめ》"),
    ("forward bouten (goma/right)", "対象［＃「対象」に傍点］"),
    ("left bouten", "X［＃「X」の左に傍点］"),
    ("tcy", "20［＃「20」は縦中横］"),
    ("gaiji", "※［＃二の字点、1-2-22］"),
    ("kaeriten", "学［＃二、レ点］而時習之"),
    ("unknown annotation", "前［＃ほげふが］後"),
    ("page break", "前\n\n［＃改ページ］\n\n後"),
    ("section break (choho)", "前\n\n［＃改丁］\n\n後"),
    ("indent leaf", "前［＃地から１字下げ］後"),
    ("align-end leaf", "前［＃地付き］末尾"),
    (
        "indent container",
        "［＃ここから字下げ］\n本文\n［＃ここで字下げ終わり］",
    ),
    (
        "align-end container",
        "［＃ここから地付き］\n後書き\n［＃ここで地付き終わり］",
    ),
    (
        "keigakomi container",
        "［＃罫囲み］\n引用\n［＃罫囲み終わり］",
    ),
    (
        "warichu (inline)",
        "黄色い鑑札（［＃割り注］淫売婦の鑑札［＃割り注終わり］）をもって",
    ),
    ("double ruby", "《《強調》》"),
];

/// `AFM_CLASSES` entries the curated corpus does not (yet) emit at the
/// current pin, each with a justification. Keep this short — an entry
/// here is a known coverage gap, not license to skip new classes.
const UNEXERCISED: &[&str] = &[
    // 割り注 renders inline (`afm-warichu`); the deprecated block form
    // `［＃ここから割り注］…` is not pinned in this corpus.
    "afm-container-warichu",
    // The leaf `aozora-indent` span is emitted by `AozoraNode::Indent`
    // (aozora-render `render_indent`), but aozora-render exercises it
    // only via a directly-constructed `Indent { amount }` node — no
    // source-string trigger is pinned, and afm has none either. The
    // *container* form (`afm-container-indent`) is exercised above.
    "afm-indent",
    // Kanbun 返り点: `aozora-kaeriten` (`<sup>`). `学［＃二、レ点］而時習之`
    // is aozora's pipeline classify input but does not surface the
    // class through afm's render path at the current pin — no verified
    // afm-source trigger. (Candidate follow-up: pin afm kaeriten render
    // coverage.)
    "afm-kaeriten",
];

/// Collect every `afm-*` class token appearing in a `class="..."`
/// attribute in `html`. (The corpus emits no `<pre><code>` blocks, so a
/// plain scan is sufficient here.)
fn collect_afm_class_tokens(html: &str, out: &mut HashSet<String>) {
    let mut rest = html;
    while let Some(i) = rest.find("class=\"") {
        let after = &rest[i + "class=\"".len()..];
        let Some(end) = after.find('"') else { break };
        for tok in after[..end].split_whitespace() {
            if tok.starts_with("afm-") {
                out.insert(tok.to_owned());
            }
        }
        rest = &after[end + 1..];
    }
}

#[test]
fn every_rendered_class_is_recognised() {
    for (label, src) in RENDER_CORPUS {
        let html = render_to_string(src);
        if let Err(violation) = check_css_class_contract(&html) {
            panic!(
                "corpus item {label:?} emitted an afm-* class not in AFM_CLASSES:\n  \
                 {violation}\n  src = {src:?}\n  html = {html}"
            );
        }
    }
}

#[test]
fn every_afm_class_is_exercised_by_the_corpus() {
    let mut emitted = HashSet::new();
    for (_label, src) in RENDER_CORPUS {
        collect_afm_class_tokens(&render_to_string(src), &mut emitted);
    }
    let exercised = |base: &str| {
        emitted
            .iter()
            .any(|t| t == base || t.starts_with(&format!("{base}-")))
    };
    let stale: Vec<&str> = AFM_CLASSES
        .iter()
        .copied()
        .filter(|base| !exercised(base) && !UNEXERCISED.contains(base))
        .collect();
    assert!(
        stale.is_empty(),
        "AFM_CLASSES entries no construct emits — remove the stale entry, \
         add a corpus source, or document it in UNEXERCISED: {stale:?}"
    );
}
