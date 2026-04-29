//! HTML post-processing: splice Aozora sentinels into rendered comrak HTML.
//!
//! The afm pipeline runs comrak verbatim against the lexer's normalized
//! text. Comrak emits ordinary `<p>...</p>` paragraphs for the lines
//! the lexer planted with PUA sentinels (U+E001..U+E004 are not in
//! CommonMark's HTML escape set, so they survive `format_html` verbatim).
//! This module rewrites that HTML so each sentinel becomes its real
//! Aozora HTML, while plain comrak output passes through unchanged.
//!
//! ## Sentinel taxonomy
//!
//! | Sentinel               | Source shape       | comrak emits            | We rewrite to                                    |
//! |------------------------|--------------------|-------------------------|---------------------------------------------------|
//! | `INLINE` (U+E001)      | inline `｜...《》` | text inside a paragraph | `aozora_render::render_node::render` of the node |
//! | `BLOCK_LEAF` (U+E002)  | leaf annotation    | `<p>U+E002</p>`         | `render_node` output (no surrounding `<p>`)      |
//! | `BLOCK_OPEN` (U+E003)  | container start    | `<p>U+E003</p>`         | `render_node` open-pass output                   |
//! | `BLOCK_CLOSE` (U+E004) | container end      | `<p>U+E004</p>`         | `render_node` close-pass output                  |
//!
//! ## Paragraph-aware splice
//!
//! v0.2.5 made the splice paragraph-aware so two further cases are
//! handled correctly:
//!
//! - **Heading promotion** — a paragraph carrying a `HeadingHint`
//!   inline sentinel (`［＃「X」は大見出し］`) becomes
//!   `<h{level}>{target}</h{level}>`. Other Aozora sentinels in the
//!   same paragraph are consumed for registry lockstep but their HTML
//!   is dropped, since the heading body is the hint's `target` field.
//! - **Stack-balanced container close** — a `BlockClose` paragraph
//!   without a matching open is silently discarded so we don't emit
//!   orphan `</div>` tags. This protects the Tier-D tag-balance
//!   invariant against pathological inputs.
//!
//! ## Order-based dispatch
//!
//! `aozora_lex` writes sentinels into `normalized` in source order,
//! and the registry tables are sorted by byte position by
//! construction. comrak preserves text order across `<p>...</p>`
//! boundaries, so the order we encounter sentinels in the rendered
//! HTML matches the order of the corresponding registry entries.
//! We therefore pre-flatten the registry into an ordered
//! `Vec<NodeRef<'_>>` keyed by source position and dispatch
//! sequentially. No byte-position lookup is needed at HTML-rewrite
//! time.

use core::fmt;

use aozora_lex::{
    BLOCK_CLOSE_SENTINEL, BLOCK_LEAF_SENTINEL, BLOCK_OPEN_SENTINEL, BorrowedLexOutput,
    INLINE_SENTINEL,
};
use aozora_render::render_node;
use aozora_syntax::borrowed::{AozoraNode, HeadingHint, NodeRef};
use aozora_syntax::{Container, ContainerKind};

/// Splice every Aozora sentinel in `comrak_html` into its real HTML
/// rendering, using the registry inside `lex_out`.
#[must_use]
pub(crate) fn splice_aozora_html(comrak_html: &str, lex_out: &BorrowedLexOutput<'_>) -> String {
    let nodes = collect_node_refs_in_normalized_order(lex_out);
    let mut state = SpliceState {
        nodes: nodes.as_slice(),
        node_idx: 0,
        container_stack: Vec::new(),
    };

    let mut out = String::with_capacity(comrak_html.len());
    splice_into(comrak_html, &mut state, &mut out);
    // Close any container that was opened but never closed in the
    // source. Without this, malformed inputs produce an HTML tree
    // with orphan `<div>` tags and Tier-D (tag balance) breaks.
    while let Some(kind) = state.container_stack.pop() {
        render_node_into(AozoraNode::Container(Container { kind }), false, &mut out);
    }
    out
}

/// Walk `normalized` in byte order; for every PUA sentinel, query the
/// registry and append the resulting [`NodeRef`] to a `Vec`.
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

struct SpliceState<'a, 'src> {
    nodes: &'a [NodeRef<'src>],
    node_idx: usize,
    /// `ContainerKind` of every still-open paired container, in LIFO
    /// order. Push on `BlockOpen`, pop on `BlockClose`. Tracking the
    /// kind (rather than just a depth counter) lets us synthesise a
    /// matching close node when the source ends without one.
    container_stack: Vec<ContainerKind>,
}

impl<'src> SpliceState<'_, 'src> {
    fn peek(&self, offset: usize) -> Option<NodeRef<'src>> {
        self.nodes.get(self.node_idx + offset).copied()
    }
    fn next(&mut self) -> Option<NodeRef<'src>> {
        let n = self.nodes.get(self.node_idx).copied();
        if n.is_some() {
            self.node_idx += 1;
        }
        n
    }
    fn advance(&mut self, n: usize) {
        self.node_idx = self.node_idx.saturating_add(n).min(self.nodes.len());
    }
}

fn splice_into(html: &str, state: &mut SpliceState<'_, '_>, out: &mut String) {
    let mut cursor = 0;
    let len = html.len();
    while cursor < len {
        // Process every `<p>...</p>` as a unit so we can handle
        // single-block-sentinel paragraphs and heading-hint
        // promotions structurally. Any inline sentinels living in
        // *other* block contexts (`<h1>`, `<li>`, `<blockquote>`,
        // table cells) flow through `splice_inline_pass`, which
        // substitutes them in place without touching the surrounding
        // tags.
        let Some(p_open_rel) = html[cursor..].find("<p>") else {
            // No more `<p>` anchors. The remainder may still contain
            // inline sentinels embedded in headings / list items /
            // tables, so finish with one inline pass.
            splice_inline_pass(&html[cursor..], state, out);
            break;
        };
        let p_open_abs = cursor + p_open_rel;

        // Region between the cursor and the next `<p>` may carry
        // inline sentinels (e.g. inside an `<h1>` body). Run an
        // inline pass instead of a verbatim copy.
        if p_open_abs > cursor {
            splice_inline_pass(&html[cursor..p_open_abs], state, out);
        }

        let after_open = p_open_abs + 3;
        let Some(close_rel) = html[after_open..].find("</p>") else {
            // Malformed markup; treat the rest as inline and bail.
            splice_inline_pass(&html[p_open_abs..], state, out);
            break;
        };
        let p_close_abs = after_open + close_rel;
        let inner = &html[after_open..p_close_abs];
        let after_close = p_close_abs + 4; // skip "</p>"

        process_paragraph(inner, state, out);
        cursor = after_close;
    }
}

fn process_paragraph(inner: &str, state: &mut SpliceState<'_, '_>, out: &mut String) {
    // Case 1: a paragraph whose body is exactly one block-sentinel
    // character. comrak isolates these because lex pads them with
    // `\n\n` (Phase 4). We replace the whole `<p>...</p>` with
    // standalone block / container HTML.
    if let Some(kind) = sole_block_sentinel(inner) {
        let Some(node_ref) = state.next() else {
            return;
        };
        match (kind, node_ref) {
            (BlockSentinelKind::Leaf, NodeRef::BlockLeaf(node)) => {
                render_node_into(node, true, out);
            }
            (BlockSentinelKind::Open, NodeRef::BlockOpen(ck)) => {
                state.container_stack.push(ck);
                render_node_into(AozoraNode::Container(Container { kind: ck }), true, out);
            }
            (BlockSentinelKind::Close, NodeRef::BlockClose(ck))
                if state.container_stack.pop().is_some() =>
            {
                // Matched open: emit the close tag.
                render_node_into(AozoraNode::Container(Container { kind: ck }), false, out);
            }
            _ => {
                // Registry/HTML drift; drop the entry.
            }
        }
        return;
    }

    // Case 2: paragraph carries a `HeadingHint` inline sentinel —
    // promote the host paragraph to `<h{level}>...</h{level}>` and
    // discard the rest of the paragraph's sentinel HTML (the heading
    // body is the hint's `target`, not the surrounding text).
    if let Some(hint) = heading_hint_in_paragraph(inner, state) {
        consume_inline_sentinels(inner, state);
        let level = hint.level.clamp(1, 6);
        write!(out, "<h{level}>").expect("writing to a String never fails");
        push_html_escaped(out, hint.target);
        write!(out, "</h{level}>").expect("writing to a String never fails");
        out.push('\n');
        return;
    }

    // Case 3: ordinary paragraph — re-emit the wrapper and substitute
    // any inline sentinels in place.
    out.push_str("<p>");
    splice_inline_pass(inner, state, out);
    out.push_str("</p>");
}

#[derive(Debug, Clone, Copy)]
enum BlockSentinelKind {
    Leaf,
    Open,
    Close,
}

/// If `inner` consists of exactly one block-sentinel character
/// (optionally surrounded by ASCII whitespace), return its kind.
fn sole_block_sentinel(inner: &str) -> Option<BlockSentinelKind> {
    let trimmed = inner.trim_matches(|c: char| matches!(c, ' ' | '\t' | '\n' | '\r'));
    let mut chars = trimmed.chars();
    let first = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    Some(match first {
        BLOCK_LEAF_SENTINEL => BlockSentinelKind::Leaf,
        BLOCK_OPEN_SENTINEL => BlockSentinelKind::Open,
        BLOCK_CLOSE_SENTINEL => BlockSentinelKind::Close,
        _ => return None,
    })
}

/// Peek the inline sentinels in this paragraph against the registry.
/// If the first inline sentinel is a `HeadingHint`, return it.
fn heading_hint_in_paragraph<'src>(
    inner: &str,
    state: &SpliceState<'_, 'src>,
) -> Option<&'src HeadingHint<'src>> {
    let mut peek_offset = 0;
    for ch in inner.chars() {
        if !is_sentinel_char(ch) {
            continue;
        }
        let node = state.peek(peek_offset)?;
        peek_offset += 1;
        if let NodeRef::Inline(AozoraNode::HeadingHint(h)) = node {
            return Some(h);
        }
    }
    None
}

/// Consume every inline-sentinel registry entry that the paragraph
/// covers. Used after a heading-hint rewrite to keep the dispatcher
/// in lockstep without emitting any of the in-paragraph nodes.
fn consume_inline_sentinels(inner: &str, state: &mut SpliceState<'_, '_>) {
    let count = inner.chars().filter(|&c| is_sentinel_char(c)).count();
    state.advance(count);
}

fn splice_inline_pass(slice: &str, state: &mut SpliceState<'_, '_>, out: &mut String) {
    let mut cursor = 0;
    for (idx, ch) in slice.char_indices() {
        if !is_sentinel_char(ch) {
            continue;
        }
        out.push_str(&slice[cursor..idx]);
        cursor = idx + ch.len_utf8();
        let Some(node_ref) = state.next() else {
            continue;
        };
        if ch == INLINE_SENTINEL {
            if let NodeRef::Inline(node) = node_ref {
                render_node_into(node, true, out);
            }
            // Mismatch (block payload at an inline position) → drop.
        } else {
            // Block sentinel inside an inline pass (e.g. inside a
            // fenced code block, where comrak emits the sentinel as
            // raw text). Drop the registry entry; emit nothing.
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

fn push_html_escaped(out: &mut String, s: &str) {
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
}

/// `fmt::Write` adapter over `&mut String`.
struct StringSink<'s>(&'s mut String);

impl fmt::Write for StringSink<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.0.write_str(s)
    }
}

// `write!` macro brings `fmt::Write` into scope.
use core::fmt::Write as _;

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

    #[test]
    fn heading_hint_promotes_paragraph_to_heading() {
        let html = render("第一篇［＃「第一篇」は大見出し］");
        assert!(
            html.contains("<h1>第一篇</h1>"),
            "expected <h1>第一篇</h1>, got {html}"
        );
    }

    #[test]
    fn orphan_close_does_not_emit_div() {
        let html = render("［＃ここで字下げ終わり］");
        let opens = html.matches("<div").count();
        let closes = html.matches("</div>").count();
        assert_eq!(opens, closes, "tag-balance broken: {html}");
    }
}
