//! Aozora notations that land in *literal* markdown contexts — inline
//! code spans and link/image destinations — must render as their original
//! source, never as an interpreted Aozora node, and must never leak the
//! internal PUA sentinel (`U+E001..=U+E004`) into the HTML.
//!
//! The lexer is CommonMark-blind (ADR-0010): it replaces every Aozora
//! notation in the text with a sentinel *before* comrak parses, so a
//! notation written inside backticks or a URL becomes a sentinel that
//! comrak then routes into a `Code` literal or a `Link.url` field — places
//! the splicer used to skip. Skipping leaked the sentinel AND desynced the
//! registry cursor, corrupting *later* notations. These tests pin the fix:
//! the splicer now rewrites such sentinels back to their original source
//! (sliced via the lexer's `source_nodes` span table) and keeps the cursor
//! in lockstep.

use aozora_flavored_markdown::html::render_to_string;
use aozora_flavored_markdown_test_support::check_no_sentinel_leak;

/// No `U+E001..=U+E004` sentinel may survive into the HTML.
fn assert_no_sentinel(html: &str) {
    if let Err(e) = check_no_sentinel_leak(html) {
        panic!("sentinel leaked: {e:?}\n  html = {html:?}");
    }
}

// ---------------------------------------------------------------------------
// Inline code spans
// ---------------------------------------------------------------------------

#[test]
fn ruby_inside_inline_code_renders_literally() {
    // `｜青梅《おうめ》` inside backticks is literal markdown: the source
    // text must appear verbatim in <code>, NOT as an interpreted <ruby>.
    let html = render_to_string("`｜青梅《おうめ》`");
    assert_no_sentinel(&html);
    assert!(
        html.contains("<code>｜青梅《おうめ》</code>"),
        "inline code must carry the literal Aozora source, got {html:?}"
    );
    assert!(
        !html.contains("<ruby>"),
        "inline code must not interpret the ruby, got {html:?}"
    );
}

#[test]
fn implicit_ruby_inside_inline_code_keeps_base_text() {
    // Implicit ruby (`青梅《おうめ》`, no `｜`): the lexer consumes the
    // base `青梅` into the notation, so the span-sliced literal must
    // restore the full original including the base.
    let html = render_to_string("`青梅《おうめ》`");
    assert_no_sentinel(&html);
    assert!(
        html.contains("<code>青梅《おうめ》</code>"),
        "implicit-ruby literal must include the base text, got {html:?}"
    );
}

#[test]
fn bouten_directive_inside_inline_code_renders_literally() {
    // An *inline* bracket directive (傍点, a forward-reference over the
    // preceding run) inside backticks stays literal source. (Block-level
    // directives like ［＃改ページ］ are `\n\n`-padded by the lexer and so
    // can't sit inside a single-line code span — that's a separate shape.)
    let html = render_to_string("`text［＃「text」に傍点］`");
    assert_no_sentinel(&html);
    assert!(
        html.contains("<code>text［＃「text」に傍点］</code>"),
        "inline directive inside inline code must stay literal, got {html:?}"
    );
}

#[test]
fn sentinel_in_inline_code_does_not_desync_following_notation() {
    // The regression that motivated the fix: a notation inside inline code
    // used to consume nothing, so the *next* real notation grabbed the
    // wrong registry entry. Here the trailing ｜B《b》 must render as B/b,
    // not as A/a from the code span.
    let html = render_to_string("`｜A《a》` then ｜B《b》end");
    assert_no_sentinel(&html);
    assert!(
        html.contains("<code>｜A《a》</code>"),
        "code span keeps its literal, got {html:?}"
    );
    assert!(
        html.contains("<ruby>B") && html.contains("<rt>b</rt>"),
        "trailing notation must render its OWN content (B/b), got {html:?}"
    );
    assert!(
        !html.contains("<ruby>A"),
        "the code span's A must not leak into a rendered ruby, got {html:?}"
    );
}

// ---------------------------------------------------------------------------
// Link / image destinations
// ---------------------------------------------------------------------------

#[test]
fn ruby_trigger_in_link_url_keeps_literal_destination() {
    // A notation inside a link URL must keep the author's literal URL
    // (comrak then percent-encodes the fullwidth chars), not a
    // percent-encoded sentinel.
    let html = render_to_string("[x](http://e.com/｜p《r》)");
    assert_no_sentinel(&html);
    // U+E001 percent-encodes to %EE%80%81; the literal ｜ is %EF%BD%9C.
    assert!(
        !html.contains("%EE%80%81"),
        "the sentinel must not survive (even percent-encoded) in the href, got {html:?}"
    );
    assert!(
        html.contains("%EF%BD%9C"),
        "the literal fullwidth ｜ should be percent-encoded in the href, got {html:?}"
    );
}

#[test]
fn notation_in_link_url_does_not_desync_link_text() {
    // The link text notation and the URL notation must each consume their
    // own registry entry in source order: text first, then url.
    let html = render_to_string("[｜T《t》](http://e.com/｜U《u》)");
    assert_no_sentinel(&html);
    // Link text renders its ruby (T/t)...
    assert!(
        html.contains("<ruby>T") && html.contains("<rt>t</rt>"),
        "link text notation must render as ruby (T/t), got {html:?}"
    );
    // ...and the URL keeps its literal (U/u not interpreted, no sentinel).
    assert!(
        html.contains("href=\"http://e.com/"),
        "link destination must be preserved, got {html:?}"
    );
}

// ---------------------------------------------------------------------------
// Fenced code blocks (already masked) stay literal — regression guard.
// ---------------------------------------------------------------------------

#[test]
fn fenced_code_block_still_literal() {
    let html = render_to_string("```\n｜青梅《おうめ》\n```");
    assert_no_sentinel(&html);
    assert!(
        html.contains("｜青梅《おうめ》"),
        "fenced code must keep its literal Aozora source, got {html:?}"
    );
    assert!(
        !html.contains("<ruby>"),
        "fenced code must not interpret the ruby, got {html:?}"
    );
}
