//! HTML post-processing: splice Aozora sentinels into rendered comrak HTML.
//!
//! The afm pipeline runs comrak verbatim against the lexer's normalized
//! text. Comrak emits ordinary `<p>...</p>` paragraphs for the lines
//! the lexer planted with PUA sentinels (U+E001..U+E004 are not in
//! CommonMark's HTML escape set, so they survive `format_html_with_options`
//! verbatim). This module rewrites that HTML so each sentinel becomes
//! its real Aozora HTML, while plain comrak output passes through
//! unchanged.
//!
//! ## Sentinel taxonomy
//!
//! | Sentinel             | Source shape       | comrak emits           | We rewrite to                                    |
//! |----------------------|--------------------|------------------------|---------------------------------------------------|
//! | `INLINE` (U+E001)      | inline `｜...《》` | text inside a paragraph | `aozora_render::render_node::render` of the node |
//! | `BLOCK_LEAF` (U+E002)  | leaf annotation    | `<p>U+E002</p>`         | `render_node` output (no surrounding `<p>`)      |
//! | `BLOCK_OPEN` (U+E003)  | container start    | `<p>U+E003</p>`         | `render_node` open-pass output                   |
//! | `BLOCK_CLOSE` (U+E004) | container end      | `<p>U+E004</p>`         | `render_node` close-pass output                  |
//!
//! ## Order-based dispatch
//!
//! `aozora_lex` writes sentinels into `normalized` in source order,
//! and the registry tables (`inline` / `block_leaf` / `block_open` /
//! `block_close`) are sorted by byte position by construction. comrak
//! preserves text order across `<p>...</p>` boundaries, so the order
//! we encounter sentinels in the rendered HTML matches the order
//! of the corresponding registry entries. We therefore pre-flatten
//! the registry into an ordered `Vec<NodeRef<'_>>` keyed by source
//! position and dispatch sequentially. No byte-position lookup is
//! needed at HTML-rewrite time.

use core::fmt;
use std::vec::IntoIter;

use aozora_lex::{
    BLOCK_CLOSE_SENTINEL, BLOCK_LEAF_SENTINEL, BLOCK_OPEN_SENTINEL, BorrowedLexOutput,
    INLINE_SENTINEL,
};
use aozora_render::render_node;
use aozora_syntax::Container;
use aozora_syntax::borrowed::{AozoraNode, NodeRef};

/// Splice every Aozora sentinel in `comrak_html` into its real HTML
/// rendering, using the registry inside `lex_out`.
#[must_use]
pub(crate) fn splice_aozora_html(comrak_html: &str, lex_out: &BorrowedLexOutput<'_>) -> String {
    let nodes = collect_node_refs_in_normalized_order(lex_out);
    let mut node_iter = nodes.into_iter();

    let mut out = String::with_capacity(comrak_html.len());
    splice_into(comrak_html, &mut node_iter, &mut out);
    out
}

/// Walk `normalized` in byte order; for every PUA sentinel, query the
/// registry and append the resulting [`NodeRef`] to a `Vec`. The
/// resulting order matches the order sentinels appear in the comrak
/// HTML output.
fn collect_node_refs_in_normalized_order<'a>(lex_out: &BorrowedLexOutput<'a>) -> Vec<NodeRef<'a>> {
    let mut out = Vec::with_capacity(lex_out.registry.len());
    for (idx, ch) in lex_out.normalized.char_indices() {
        let is_sentinel = matches!(
            ch,
            INLINE_SENTINEL | BLOCK_LEAF_SENTINEL | BLOCK_OPEN_SENTINEL | BLOCK_CLOSE_SENTINEL
        );
        if !is_sentinel {
            continue;
        }
        let pos = u32::try_from(idx).expect("normalized text fits u32 (Phase 0 cap)");
        if let Some(node_ref) = lex_out.registry.node_at(pos) {
            out.push(node_ref);
        }
    }
    out
}

fn splice_into(comrak_html: &str, nodes: &mut IntoIter<NodeRef<'_>>, out: &mut String) {
    // Two-stage loop:
    //   (1) detect block-sentinel paragraphs (`<p>U+E0xx</p>`) and
    //       rewrite them as standalone block / container HTML.
    //   (2) fall back to a per-character pass for everything else;
    //       inline sentinels get inlined into the surrounding
    //       paragraph.
    //
    // Block-sentinel paragraphs are line-anchored by lex's `\n\n`
    // padding (Phase 4 inserts a leading + trailing blank line around
    // every block sentinel), so comrak emits each as an isolated
    // `<p>U+E0xx</p>\n` line.
    let mut cursor = 0;
    let bytes_len = comrak_html.len();
    while cursor < bytes_len {
        if let Some(matched) = match_block_paragraph(comrak_html, cursor) {
            if matched.start > cursor {
                splice_inline_pass(&comrak_html[cursor..matched.start], nodes, out);
            }
            let node_ref = nodes.next().expect(
                "registry order matches comrak output order: block sentinel without registry entry",
            );
            render_block(node_ref, matched.kind, out);
            cursor = matched.end;
            continue;
        }
        // No block-sentinel paragraph here; sweep up to the next
        // newline (or EOF) and run the inline pass over that slice.
        let next_break = comrak_html[cursor..]
            .find('\n')
            .map_or(bytes_len, |off| cursor + off + 1);
        splice_inline_pass(&comrak_html[cursor..next_break], nodes, out);
        cursor = next_break;
    }
}

/// One-line block-sentinel paragraph match. Returns `Some` if
/// `comrak_html[from..]` starts with `<p>U+E0xx</p>` (with optional
/// leading ASCII whitespace tolerated for indented contexts).
fn match_block_paragraph(comrak_html: &str, from: usize) -> Option<BlockMatch> {
    let tail = &comrak_html[from..];
    let prefix_ws_len = tail
        .bytes()
        .take_while(|b| matches!(b, b' ' | b'\t'))
        .count();
    let body = &tail[prefix_ws_len..];
    if !body.starts_with("<p>") {
        return None;
    }
    let after_open = &body[3..];
    let sentinel_char = after_open.chars().next()?;
    let kind = match sentinel_char {
        BLOCK_LEAF_SENTINEL => BlockSentinelKind::Leaf,
        BLOCK_OPEN_SENTINEL => BlockSentinelKind::Open,
        BLOCK_CLOSE_SENTINEL => BlockSentinelKind::Close,
        _ => return None,
    };
    let after_sentinel_off = sentinel_char.len_utf8();
    let rest = &after_open[after_sentinel_off..];
    if !rest.starts_with("</p>") {
        return None;
    }
    let close_off = after_sentinel_off + 4;
    Some(BlockMatch {
        start: from,
        end: from + prefix_ws_len + 3 + close_off,
        kind,
    })
}

#[derive(Debug, Clone, Copy)]
struct BlockMatch {
    start: usize,
    end: usize,
    kind: BlockSentinelKind,
}

#[derive(Debug, Clone, Copy)]
enum BlockSentinelKind {
    Leaf,
    Open,
    Close,
}

fn render_block(node_ref: NodeRef<'_>, expected: BlockSentinelKind, out: &mut String) {
    match (expected, node_ref) {
        (BlockSentinelKind::Leaf, NodeRef::BlockLeaf(node)) => {
            render_node_into(node, true, out);
        }
        (BlockSentinelKind::Open, NodeRef::BlockOpen(kind)) => {
            render_node_into(AozoraNode::Container(Container { kind }), true, out);
        }
        (BlockSentinelKind::Close, NodeRef::BlockClose(kind)) => {
            render_node_into(AozoraNode::Container(Container { kind }), false, out);
        }
        (kind, got) => {
            // Registry order drift would be a hard bug. Fail loud in
            // tests; in release builds we drop the mismatched entry
            // so HTML well-formedness invariants downstream still
            // catch the symptom.
            debug_assert!(
                false,
                "block sentinel kind/registry mismatch: expected {kind:?}, got {got:?}"
            );
        }
    }
}

/// Process a slice that may contain inline sentinels (and ordinary
/// text). Block sentinels should not appear here — `splice_into`
/// extracts those upstream — but we tolerate stray ones by dropping
/// the corresponding registry entry to keep the dispatch in lockstep.
fn splice_inline_pass(slice: &str, nodes: &mut IntoIter<NodeRef<'_>>, out: &mut String) {
    let mut cursor = 0;
    for (idx, ch) in slice.char_indices() {
        if !is_sentinel_char(ch) {
            continue;
        }
        out.push_str(&slice[cursor..idx]);
        cursor = idx + ch.len_utf8();
        if ch == INLINE_SENTINEL {
            let node_ref = nodes.next().expect(
                "registry order matches comrak output order: inline sentinel without registry entry",
            );
            if let NodeRef::Inline(node) = node_ref {
                render_node_into(node, true, out);
            } else {
                debug_assert!(false, "inline sentinel position holds non-inline NodeRef");
            }
        } else {
            debug_assert!(
                false,
                "block sentinel found in inline pass — block-paragraph match should have caught it"
            );
            let _ = nodes.next();
        }
    }
    out.push_str(&slice[cursor..]);
}

fn is_sentinel_char(ch: char) -> bool {
    matches!(
        ch,
        INLINE_SENTINEL | BLOCK_LEAF_SENTINEL | BLOCK_OPEN_SENTINEL | BLOCK_CLOSE_SENTINEL
    )
}

fn render_node_into(node: AozoraNode<'_>, entering: bool, out: &mut String) {
    render_node::render(node, entering, &mut StringSink(out))
        .expect("writing AozoraNode HTML to a String cannot fail");
}

/// `fmt::Write` adapter over `&mut String`.
struct StringSink<'s>(&'s mut String);

impl fmt::Write for StringSink<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.0.write_str(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aozora_syntax::borrowed::Arena;

    fn render(input: &str) -> String {
        let arena = Arena::new();
        let lex_out = aozora_lex::lex_into_arena(input, &arena);
        let comrak_arena = comrak::Arena::new();
        let opts = comrak::Options::default();
        let root = comrak::parse_document(&comrak_arena, lex_out.normalized, &opts);
        let mut html = String::new();
        comrak::format_html(root, &opts, &mut html).unwrap();
        splice_aozora_html(&html, &lex_out)
    }

    #[test]
    fn plain_text_passes_through() {
        assert!(render("hello").contains("hello"));
    }

    #[test]
    fn ruby_inline_sentinel_is_replaced() {
        let html = render("｜青梅《おうめ》");
        assert!(html.contains("<ruby>"), "html: {html}");
        assert!(html.contains("青梅"));
        assert!(html.contains("おうめ"));
        assert!(!html.contains(INLINE_SENTINEL));
    }

    #[test]
    fn page_break_block_leaf_replaces_paragraph() {
        let html = render("前\n\n［＃改ページ］\n\n後");
        assert!(!html.contains(BLOCK_LEAF_SENTINEL));
        assert!(!html.contains("<p>\u{E002}</p>"));
    }
}
