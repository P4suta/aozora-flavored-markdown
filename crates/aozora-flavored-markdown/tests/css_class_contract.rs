//! Class-contract test between renderer and themes.
//!
//! The aozora-flavored-markdown-book ships two CSS themes (`aozora-md-horizontal.css` and
//! `aozora-md-vertical.css`) whose class selectors must cover every
//! class token the renderer can emit. Without this contract a
//! renderer change (e.g. a new `aozora-md-bouten-foo` kind) silently
//! ships unstyled markup — invisible in unit tests because the
//! existing HTML assertions don't care about CSS.
//!
//! The test reads both CSS files at runtime, extracts every
//! `.aozora-md-*` selector, and asserts each pinned class token appears
//! in both. If a renderer adds a class, the pinned list in this
//! file flags the missing style at `cargo test` time.
//!
//! The pinned list is intentionally hand-curated (not derived by
//! scraping `html.rs`) so that adding a new class forces an
//! *explicit* update here — drive-by CSS gaps can't slip in.

use core::str;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use aozora_flavored_markdown_test_support::AOZORA_MD_CLASSES;

/// Absolute path to one of the theme CSS files. Resolving via
/// `CARGO_MANIFEST_DIR` keeps the test stable regardless of the
/// runner's working directory.
fn theme_path(name: &str) -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop(); // crates/aozora-flavored-markdown → crates/
    p.push("aozora-flavored-markdown-book");
    p.push("theme");
    p.push(name);
    p
}

/// Return every `.aozora-md-…` class selector name appearing in `css`.
///
/// A tokeniser that extracts the identifier immediately after each
/// `.aozora-md-` prefix. Accepts lowercase ASCII letters, digits, and
/// hyphens; stops at any other character. Intentionally trivial —
/// the project's CSS doesn't use namespace prefixes or escaped
/// selectors.
fn collect_class_selectors(css: &str) -> HashSet<String> {
    let mut out = HashSet::new();
    let bytes = css.as_bytes();
    let mut i = 0usize;
    while i + 11 <= bytes.len() {
        if &bytes[i..i + 11] == b".aozora-md-" {
            let start = i + 1; // after the '.'
            let mut end = start;
            while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'-') {
                end += 1;
            }
            // Trim trailing hyphens from a ".aozora-md-" prefix with no
            // body — shouldn't occur but be tolerant.
            let token = str::from_utf8(&bytes[start..end]).expect("ASCII");
            if token.len() > "aozora-md-".len() && !token.ends_with('-') {
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
    let css = fs::read_to_string(theme_path("aozora-md-horizontal.css")).expect(
        "aozora-md-horizontal.css must exist alongside aozora-flavored-markdown-book/theme/",
    );
    let selectors = collect_class_selectors(&css);
    let missing: Vec<&&str> = AOZORA_MD_CLASSES
        .iter()
        .filter(|c| !selectors.contains(**c))
        .collect();
    assert!(
        missing.is_empty(),
        "aozora-md-horizontal.css is missing rules for emitted classes: {missing:?}"
    );
}

#[test]
fn every_emitted_class_has_a_vertical_theme_rule() {
    let css = fs::read_to_string(theme_path("aozora-md-vertical.css"))
        .expect("aozora-md-vertical.css must exist alongside aozora-flavored-markdown-book/theme/");
    let selectors = collect_class_selectors(&css);
    let missing: Vec<&&str> = AOZORA_MD_CLASSES
        .iter()
        .filter(|c| !selectors.contains(**c))
        .collect();
    assert!(
        missing.is_empty(),
        "aozora-md-vertical.css is missing rules for emitted classes: {missing:?}"
    );
}

#[test]
fn pinned_classes_are_sorted_and_unique() {
    // Hygiene: the pinned list is kept in sorted order + no dupes
    // for review friendliness. A sorted list also makes PR diffs
    // trivial to review.
    let mut copy: Vec<&str> = AOZORA_MD_CLASSES.to_vec();
    copy.sort_unstable();
    assert_eq!(
        AOZORA_MD_CLASSES.to_vec(),
        copy,
        "AOZORA_MD_CLASSES must stay sorted"
    );
    let mut seen: HashSet<&str> = HashSet::new();
    for &c in AOZORA_MD_CLASSES {
        assert!(seen.insert(c), "duplicate entry in AOZORA_MD_CLASSES: {c}");
    }
}

#[test]
fn collect_class_selectors_extracts_basic_rules() {
    // Self-test for the tokeniser — a regression here would
    // silently weaken every other test in this file.
    let css = ".aozora-md-foo { color: red; }\n.aozora-md-bar-baz, .aozora-md-qux { }\n.foo { }";
    let selectors = collect_class_selectors(css);
    assert!(selectors.contains("aozora-md-foo"));
    assert!(selectors.contains("aozora-md-bar-baz"));
    assert!(selectors.contains("aozora-md-qux"));
    assert!(!selectors.contains("foo"));
}

#[test]
fn collect_class_selectors_tolerates_trailing_hyphen() {
    // `.aozora-md-` alone (no body) must not emit a token.
    let css = ".aozora-md- { }";
    let selectors = collect_class_selectors(css);
    assert!(selectors.is_empty());
}
