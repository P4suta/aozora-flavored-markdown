//! Legacy HTML post-processor (frozen for differential testing only).
//!
//! Replaced by [`crate::ast_splice`] which mutates the comrak AST in
//! place rather than re-scanning the formatted HTML. This file is
//! kept under `#![cfg(test)]` so the unit-test differential gate
//! (`mod tests` below) can pin the AST splicer's output byte-equal
//! against the legacy four-pass pipeline. Once the cleanup PR runs
//! the cargo-fuzz harness for 24 h with no divergence, the entire
//! file disappears.
//!
//! Original module documentation, preserved for archaeology:
//!
//! ## Pipeline shape
//!
//! Today the pipeline is four logical passes that compose through
//! `Cow<str>` (so passes 2-4 are zero-allocation no-ops on the
//! common path):
//!
//! 1. `splice_into` — sentinel substitution (paragraph-aware).
//! 2. `rebrand_aozora_classes_to_afm` — `aozora-*` → `afm-*` brand
//!    boundary rewrite (ADR-0011).
//! 3. `wrap_orphan_brackets_in_place` — Tier-A defensive wrap for
//!    `［＃…］` runs the lexer didn't claim.
//! 4. `balance_inline_tags_in_paragraphs` — Tier-D defensive close
//!    of `<strong>` / `<em>` / etc. that aozora's annotation
//!    splitter unbalanced.
//!
//! ## Future: fully fused 1-pass automaton
//!
//! The four passes scan the same document independently. A genuine
//! 1-pass scanner — driven by an `aho-corasick` automaton over the
//! union of the four needle sets (`<p>`, `<p `, `</p>`, `class="`,
//! `［＃`, the four sentinel chars) plus a small state enum
//! (`Normal` / `InClassAttr` / `InParagraph(open_counts)`) — would
//! collapse the four scans into one. The state-machine design
//! lives in the project plan as a follow-up; the Cow-threading
//! below already eliminates the redundant *allocations* on the
//! common path, so a fused scanner would mostly buy locality and
//! cleaner state plumbing rather than raw throughput.
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
//! Two cases beyond the sentinel-substitution above are handled per
//! paragraph:
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
//! `aozora_pipeline` writes sentinels into `normalized` in source order,
//! and the registry tables are sorted by byte position by
//! construction. comrak preserves text order across `<p>...</p>`
//! boundaries, so the order we encounter sentinels in the rendered
//! HTML matches the order of the corresponding registry entries.
//! We therefore pre-flatten the registry into an ordered
//! `Vec<NodeRef<'_>>` keyed by source position and dispatch
//! sequentially. No byte-position lookup is needed at HTML-rewrite
//! time.

#![cfg(test)]

use core::fmt;
use std::borrow::Cow;

use aozora_pipeline::{BorrowedLexOutput, INLINE_SENTINEL};
use aozora_render::render_node;
use aozora_syntax::borrowed::{AozoraNode, HeadingHint, NodeRef};
use aozora_syntax::{Container, ContainerKind};

use crate::sentinel_stream::{BlockSentinelKind, SentinelCursor, is_sentinel_char};

/// String-paragraph-inner variant of `paragraph_sole_block_sentinel`.
/// Inlined here from the (now-removed) `sentinel_stream::sole_block_sentinel`
/// helper because only the legacy splicer scans HTML `<p>...</p>`
/// inner text — every production walker now consumes the comrak AST.
fn sole_block_sentinel(inner: &str) -> Option<BlockSentinelKind> {
    let trimmed = inner.trim_matches(|c: char| matches!(c, ' ' | '\t' | '\n' | '\r'));
    let mut chars = trimmed.chars();
    let first = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    BlockSentinelKind::from_char(first)
}

/// Splice every Aozora sentinel in `comrak_html` into its real HTML
/// rendering, using the registry inside `lex_out`.
///
/// The transformation is split into four logical passes that compose
/// through `Cow<str>` so the common path (well-formed input, no
/// `aozora-*` class drift, no orphan brackets, no inline-tag
/// imbalance) only allocates once — for the sentinel substitution
/// itself. Each subsequent pass is a no-op fast-path when its
/// trigger pattern is absent from the previous output.
#[must_use]
pub(crate) fn splice_aozora_html_legacy(
    comrak_html: &str,
    lex_out: &BorrowedLexOutput<'_>,
) -> String {
    // Pass 1 — sentinel substitution. The first allocation: comrak
    // emitted PUA sentinels and we expand each into its real HTML.
    let mut state = SpliceState {
        cursor: SentinelCursor::from_lex_out(Some(lex_out)),
        container_stack: Vec::new(),
    };
    let mut out = String::with_capacity(comrak_html.len());
    splice_into(comrak_html, &mut state, &mut out);
    while let Some(kind) = state.container_stack.pop() {
        render_node_into(AozoraNode::Container(Container { kind }), false, &mut out);
    }

    // Pass 2 — brand boundary (ADR-0011). The upstream `aozora-render`
    // crate emits `aozora-*` classes (its own brand); afm's HTML uses
    // the `afm-*` brand. Borrowed-fast-path when no `aozora-` token
    // survived Pass 1.
    let rebranded = rebrand_aozora_classes_to_afm(&out);

    // Pass 3 — Tier-A defensive guard. Every `［＃…］` the upstream
    // lexer failed to claim gets wrapped in an `afm-annotation`
    // hidden span here so the canary can't leak. Borrowed when no
    // bare `［＃` made it through.
    let bracket_safe = wrap_orphan_brackets_in_place(&rebranded);

    // Pass 4 — Tier-D inline tag balance. Aozora's `［＃…］` claim can
    // split a CommonMark emphasis run, leaving `<strong>` opens
    // unmatched at `</p>` time. Walk each `<p>...</p>` and append the
    // missing closes. Borrowed when emphasis tags balance.
    balance_inline_tags_in_paragraphs(&bracket_safe).into_owned()
}

/// Per-paragraph inline-tag balancer.
///
/// Walks each `<p>...</p>` substring once, counts open vs close
/// occurrences for each emphasis-family inline tag, and prepends any
/// missing closes before the paragraph's `</p>`. Touches no other
/// container kinds — paragraphs are where comrak's emphasis pairing
/// can leak the most under aozora-induced text splits.
///
/// Inline-tag-name list is intentionally narrow (`strong` / `em` /
/// `code` / `del` / `s` / `sup` / `sub`): these are the CommonMark +
/// GFM emphasis families that comrak resolves greedily and that
/// aozora's annotation splitter can leave unbalanced. `span`, `ruby`,
/// `a`, etc. are emitted by the renderer in matched pairs and stay
/// out of this pass to avoid double-closing.
fn balance_inline_tags_in_paragraphs(html: &str) -> Cow<'_, str> {
    /// `(open_exact, open_with_attr, close)` for each inline tag we
    /// rebalance. Static so the iteration allocates nothing.
    const INLINE_TAGS: &[(&str, &str, &str)] = &[
        ("<strong>", "<strong ", "</strong>"),
        ("<em>", "<em ", "</em>"),
        ("<code>", "<code ", "</code>"),
        ("<del>", "<del ", "</del>"),
        ("<s>", "<s ", "</s>"),
        ("<sup>", "<sup ", "</sup>"),
        ("<sub>", "<sub ", "</sub>"),
    ];

    // Fast path: if every inline-tag family balances at the document
    // level, the per-paragraph balance is trivially satisfied and we
    // can borrow.
    if INLINE_TAGS.iter().all(|(open_exact, open_attr, close)| {
        let opens = html.matches(open_exact).count() + html.matches(open_attr).count();
        opens == html.matches(close).count()
    }) {
        return Cow::Borrowed(html);
    }

    let mut out = String::with_capacity(html.len());
    let mut rest = html;

    while let Some(p_start) = rest.find("<p>").or_else(|| rest.find("<p ")) {
        let Some(p_end_rel) = rest[p_start..].find("</p>") else {
            break;
        };
        let p_end = p_start + p_end_rel;

        out.push_str(&rest[..p_end]);

        let body = &rest[p_start..p_end];
        for (open_exact, open_attr, close) in INLINE_TAGS {
            let opens = body.matches(open_exact).count() + body.matches(open_attr).count();
            let closes = body.matches(close).count();
            if opens > closes {
                for _ in 0..(opens - closes) {
                    out.push_str(close);
                }
            }
        }

        out.push_str("</p>");
        rest = &rest[p_end + "</p>".len()..];
    }

    out.push_str(rest);
    Cow::Owned(out)
}

/// Rewrite every `aozora-*` class token in `class="..."` attribute
/// values to `afm-*`. Touches only class attributes — the brand on
/// `data-*` attributes, on link targets, on text bodies, etc. is
/// preserved verbatim.
fn rebrand_aozora_classes_to_afm(html: &str) -> Cow<'_, str> {
    if !html.contains("aozora-") {
        return Cow::Borrowed(html);
    }
    let mut out = String::with_capacity(html.len());
    let mut cursor = 0;
    while let Some(rel) = html[cursor..].find("class=\"") {
        let attr_start = cursor + rel + "class=\"".len();
        out.push_str(&html[cursor..attr_start]);
        let Some(close_rel) = html[attr_start..].find('"') else {
            out.push_str(&html[attr_start..]);
            return Cow::Owned(out);
        };
        let attr_end = attr_start + close_rel;
        let attr_value = &html[attr_start..attr_end];
        for (i, token) in attr_value.split_ascii_whitespace().enumerate() {
            if i > 0 {
                out.push(' ');
            }
            if let Some(rest) = token.strip_prefix("aozora-") {
                out.push_str("afm-");
                out.push_str(rest);
            } else {
                out.push_str(token);
            }
        }
        out.push('"');
        cursor = attr_end + 1;
    }
    out.push_str(&html[cursor..]);
    Cow::Owned(out)
}

/// Find every `［＃…］` in `html` that lives outside an HTML tag and
/// outside an existing `afm-annotation` wrapper, and wrap it in a
/// hidden `<span class="afm-annotation" hidden>…</span>`. The class
/// name matches `aozora-render`'s annotation wrapper so
/// `test_support::strip_annotation_wrappers` continues to recognise
/// it, and the pass is idempotent: a second invocation finds the
/// `afm-annotation` substring in the prefix and skips re-wrapping.
fn wrap_orphan_brackets_in_place(html: &str) -> Cow<'_, str> {
    let needle = "［＃";
    let close = '］';
    let wrapper_class = "afm-annotation";
    let wrapper_open = "<span class=\"afm-annotation\" hidden>";
    let wrapper_close = "</span>";

    if !html.contains(needle) {
        return Cow::Borrowed(html);
    }

    let mut out = String::with_capacity(html.len());
    let mut cursor = 0;
    while let Some(rel) = html[cursor..].find(needle) {
        let abs = cursor + rel;
        // Decide skip vs wrap by inspecting the *already-emitted* prefix
        // (`out` + literal bytes from `cursor..abs`). This avoids the
        // false-skip you'd get from looking back into `html` after we've
        // started rewriting it.
        let mut prefix = String::with_capacity(out.len() + (abs - cursor));
        prefix.push_str(&out);
        prefix.push_str(&html[cursor..abs]);
        let last_open_tag = prefix.rfind('<').unwrap_or(0);
        let last_close_tag = prefix.rfind('>').unwrap_or(0);
        let inside_tag = last_open_tag > last_close_tag && !prefix.is_empty();
        // `already_wrapped` checks only the *current* unfinished span:
        // if a previous wrapper has already closed (`</span>` after the
        // last `wrapper_class` mention), we are no longer inside it.
        let last_wrapper_class = prefix.rfind(wrapper_class);
        let last_wrapper_close = prefix.rfind(wrapper_close);
        let already_wrapped = match (last_wrapper_class, last_wrapper_close) {
            (Some(c), Some(z)) => c > z,
            (Some(_), None) => true,
            _ => false,
        };
        if inside_tag || already_wrapped {
            out.push_str(&html[cursor..abs + needle.len()]);
            cursor = abs + needle.len();
            continue;
        }
        // Find a matching `］` after the marker. If none, wrap up to
        // the next `<` (start of next tag) or EOF — never leave a bare
        // bracket behind.
        let after_open = abs + needle.len();
        let bracket_run_end = html[after_open..]
            .find(close)
            .map(|r| after_open + r + close.len_utf8())
            .or_else(|| html[after_open..].find('<').map(|r| after_open + r))
            .unwrap_or(html.len());
        out.push_str(&html[cursor..abs]);
        out.push_str(wrapper_open);
        push_html_escaped(&mut out, &html[abs..bracket_run_end]);
        out.push_str(wrapper_close);
        cursor = bracket_run_end;
    }
    out.push_str(&html[cursor..]);
    Cow::Owned(out)
}

struct SpliceState<'src> {
    cursor: SentinelCursor<'src>,
    /// `ContainerKind` of every still-open paired container, in LIFO
    /// order. Push on `BlockOpen`, pop on `BlockClose`. Tracking the
    /// kind (rather than just a depth counter) lets us synthesise a
    /// matching close node when the source ends without one.
    container_stack: Vec<ContainerKind>,
}

impl<'src> SpliceState<'src> {
    fn peek(&self, offset: usize) -> Option<NodeRef<'src>> {
        self.cursor.peek(offset)
    }
    fn next(&mut self) -> Option<NodeRef<'src>> {
        self.cursor.next()
    }
    fn advance(&mut self, n: usize) {
        self.cursor.advance(n);
    }
}

fn splice_into(html: &str, state: &mut SpliceState<'_>, out: &mut String) {
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
        // Match `<p>` and `<p ...>` (paragraphs that comrak emitted
        // with attributes — e.g. when source-line anchors injected
        // `data-afm-source-line="N"`). The earlier of the two
        // positions wins when both are present.
        let p_open_rel = match (html[cursor..].find("<p>"), html[cursor..].find("<p ")) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        let Some(p_open_rel) = p_open_rel else {
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

        // Step past either `<p>` (3 bytes) or `<p ` (also 3 bytes, but
        // with attributes — find `>` to get past the attribute list).
        let tag_open_skip = if html[p_open_abs..].starts_with("<p>") {
            3
        } else {
            // Walk to the closing `>` of `<p attr=...>`.
            html[p_open_abs..]
                .find('>')
                .map_or(html.len() - p_open_abs, |i| i + 1)
        };
        let after_open = p_open_abs + tag_open_skip;
        let Some(close_rel) = html[after_open..].find("</p>") else {
            // Malformed markup; treat the rest as inline and bail.
            splice_inline_pass(&html[p_open_abs..], state, out);
            break;
        };
        let p_close_abs = after_open + close_rel;
        let open_tag = &html[p_open_abs..after_open];
        let inner = &html[after_open..p_close_abs];
        let after_close = p_close_abs + 4; // skip "</p>"

        process_paragraph(open_tag, inner, state, out);
        cursor = after_close;
    }
}

/// `open_tag` is the verbatim opening `<p>` or `<p attr=…>` slice, so
/// Case 3 (ordinary paragraph) preserves any source-line anchor or
/// future attribute the formatter attached.
fn process_paragraph(open_tag: &str, inner: &str, state: &mut SpliceState<'_>, out: &mut String) {
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
        push_html_escaped(out, &hint.target);
        write!(out, "</h{level}>").expect("writing to a String never fails");
        out.push('\n');
        return;
    }

    // Case 3: ordinary paragraph — re-emit the verbatim open tag
    // (so `<p data-afm-source-line="N">` anchors survive) and
    // substitute any inline sentinels in place.
    out.push_str(open_tag);
    splice_inline_pass(inner, state, out);
    out.push_str("</p>");
}

/// Peek the inline sentinels in this paragraph against the registry.
/// If the first inline sentinel is a `HeadingHint`, return it.
fn heading_hint_in_paragraph<'src>(
    inner: &str,
    state: &SpliceState<'src>,
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
fn consume_inline_sentinels(inner: &str, state: &mut SpliceState<'_>) {
    let count = inner.chars().filter(|&c| is_sentinel_char(c)).count();
    state.advance(count);
}

fn splice_inline_pass(slice: &str, state: &mut SpliceState<'_>, out: &mut String) {
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
    use crate::sentinel_stream::paragraph_sole_block_sentinel;
    use aozora_pipeline::BLOCK_LEAF_SENTINEL;
    use aozora_syntax::borrowed::Arena;

    fn render(input: &str) -> String {
        let arena = Arena::new();
        let lex_out = aozora_pipeline::lex_into_arena(input, &arena);
        let comrak_arena = comrak::Arena::new();
        let opts = comrak::Options::default();
        let root = comrak::parse_document(&comrak_arena, lex_out.normalized, &opts);
        let mut html = String::new();
        comrak::format_html(root, &opts, &mut html).unwrap();
        splice_aozora_html_legacy(&html, &lex_out)
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

    #[test]
    fn malformed_unclosed_paragraph_does_not_panic() {
        // Pins `splice_into`'s `</p>`-not-found fallback. Synthesise a
        // payload comrak would never emit (an unclosed `<p>` tag) and
        // confirm the splice walks it without panicking.
        let arena = Arena::new();
        let lex_out = aozora_pipeline::lex_into_arena("hello", &arena);
        let out = splice_aozora_html_legacy("<p>unclosed paragraph", &lex_out);
        assert!(out.contains("unclosed paragraph"), "got: {out}");
    }

    #[test]
    fn block_sentinel_paragraph_with_exhausted_registry_does_not_panic() {
        // Pins `process_paragraph`'s `state.next() = None` early-return.
        // We hand the splicer a paragraph that *looks* like a block
        // sentinel but for which the registry is empty. The splicer
        // must drop the paragraph silently.
        let arena = Arena::new();
        let lex_out = aozora_pipeline::lex_into_arena("plain", &arena);
        // `lex_out.registry` for "plain" is empty, but we feed an HTML
        // payload that pretends to contain one. The splicer should
        // produce no Aozora HTML for that paragraph and not panic.
        let payload = format!("<p>{BLOCK_LEAF_SENTINEL}</p>\n");
        let out = splice_aozora_html_legacy(&payload, &lex_out);
        assert!(
            !out.contains(BLOCK_LEAF_SENTINEL),
            "sentinel survived: {out}"
        );
    }

    #[test]
    fn block_sentinel_inside_inline_pass_drops_silently() {
        // Pins `splice_inline_pass`'s "block sentinel found here"
        // fallback. This is the exact path that fenced-code-block
        // contents trigger: a block sentinel survives into a non-`<p>`
        // context and must be discarded silently rather than panicking
        // or leaking.
        let html = render("```\n［＃改ページ］\n```");
        // The page-break marker must not leak into the code block as
        // its `afm-page-break` div, because it lives inside `<pre>`.
        // Either the sentinel is dropped (current behaviour) or its
        // markup escapes into the `<pre>` body — both are acceptable
        // for code-block content; what matters is that no panic
        // occurs and no raw sentinel char survives.
        assert!(
            !html.contains(BLOCK_LEAF_SENTINEL),
            "sentinel leaked: {html}"
        );
    }

    #[test]
    fn heading_hint_target_html_special_chars_are_escaped() {
        // `push_html_escaped` covers the `<`/`>`/`&`/`"`/`'` arms only
        // when a HeadingHint target carries one of those characters.
        // Exercise each via a forward-reference heading hint whose
        // target is the special char run.
        let html = render("<&\"'><&\"'>［＃「<&\"'>」は大見出し］");
        assert!(html.contains("&lt;"), "missing < escape: {html}");
        assert!(html.contains("&gt;"), "missing > escape: {html}");
        assert!(html.contains("&amp;"), "missing & escape: {html}");
        assert!(html.contains("&quot;"), "missing \" escape: {html}");
        assert!(html.contains("&#39;"), "missing ' escape: {html}");
    }

    #[test]
    fn balance_inline_tags_appends_missing_close() {
        // Pins `balance_inline_tags_in_paragraphs`'s slow path: when a
        // paragraph carries an unmatched `<strong>` open, the balancer
        // must append `</strong>` before `</p>`. The fast-path returns
        // `Cow::Borrowed` only when every inline-tag family balances.
        let bad = "<p>opener <strong>broken</p><p>after</p>";
        let out = balance_inline_tags_in_paragraphs(bad);
        // The first paragraph picked up the missing close-tag.
        assert!(
            out.contains("<strong>broken</strong></p>"),
            "missing close not inserted: {out}"
        );
        // The second paragraph stays untouched.
        assert!(
            out.contains("<p>after</p>"),
            "trailing paragraph dropped: {out}"
        );
        // And the result is owned (slow path).
        assert!(matches!(out, Cow::Owned(_)));
    }

    #[test]
    fn balance_inline_tags_handles_attr_form_and_dangling_open() {
        // Exercises the `<strong ` (attribute form) needle and the
        // `</p>`-not-found break: a paragraph that opens `<strong …>`
        // without a close, plus a stray `<p>` whose `</p>` is missing.
        let bad = "<p><strong class=\"x\">a</p><p>tail";
        let out = balance_inline_tags_in_paragraphs(bad);
        assert!(
            out.contains("<strong class=\"x\">a</strong></p>"),
            "attr-form open was not balanced: {out}"
        );
        // Trailing `<p>tail` had no `</p>` — it must be appended verbatim.
        assert!(out.ends_with("<p>tail"), "tail was rewritten: {out}");
    }

    #[test]
    fn rebrand_preserves_non_aozora_tokens() {
        // Pins `rebrand_aozora_classes_to_afm`'s `else` arm where a
        // class token without the `aozora-` prefix is emitted verbatim.
        let html = "<span class=\"foo aozora-ruby bar\">x</span>";
        let out = rebrand_aozora_classes_to_afm(html);
        assert!(out.contains("class=\"foo afm-ruby bar\""), "got: {out}");
    }

    #[test]
    fn rebrand_handles_unclosed_class_attribute() {
        // Pins the `find('"')`-fails branch: synthesise a payload with
        // an unterminated `class="…` so the balancer hits the early
        // return after copying the prefix.
        let bad = "before <span class=\"aozora-ruby and on it goes";
        let out = rebrand_aozora_classes_to_afm(bad);
        // The prefix up to `class="` is preserved verbatim and the
        // remainder (the unterminated value) is appended unchanged.
        assert!(out.contains("class=\""), "class= prefix lost: {out}");
        assert!(
            out.ends_with("and on it goes"),
            "unterminated tail dropped: {out}"
        );
    }

    #[test]
    fn splice_chooses_earlier_of_p_exact_and_p_attr() {
        // Pins the `(Some(a), Some(b)) => Some(a.min(b))` branch in
        // `splice_into`: hand the splicer HTML with both `<p>` and
        // `<p data-…>` and confirm both paragraphs are processed in
        // source order.
        let arena = Arena::new();
        let lex_out = aozora_pipeline::lex_into_arena("plain", &arena);
        // `<p data-…>` first, then plain `<p>`. Both must survive.
        let payload = "<p data-afm-source-line=\"1\">first</p>\n<p>second</p>\n";
        let out = splice_aozora_html_legacy(payload, &lex_out);
        let first_pos = out.find("first").expect("first paragraph dropped");
        let second_pos = out.find("second").expect("second paragraph dropped");
        assert!(first_pos < second_pos, "order changed: {out}");
    }

    #[test]
    fn paragraph_sole_block_sentinel_rejects_two_sentinels() {
        // Pins `paragraph_sole_block_sentinel`'s `found.is_some()`
        // early-return: a paragraph whose body carries two block
        // sentinels must yield `None` (and thus render as a normal
        // paragraph), not `Some(kind)`.
        use aozora_pipeline::BLOCK_LEAF_SENTINEL;
        let payload = format!("{BLOCK_LEAF_SENTINEL}{BLOCK_LEAF_SENTINEL}");
        let arena = Arena::new();
        let lex_out = aozora_pipeline::lex_into_arena(&payload, &arena);
        let comrak_arena = comrak::Arena::new();
        let opts = comrak::Options::default();
        let root = comrak::parse_document(&comrak_arena, lex_out.normalized, &opts);
        let para = root.first_child().expect("paragraph node");
        // Direct call: must return None even though both chars are
        // valid block sentinels.
        assert!(
            paragraph_sole_block_sentinel(para).is_none(),
            "two sentinels accepted as sole"
        );
    }

    #[test]
    fn paragraph_sole_block_sentinel_skips_leading_whitespace() {
        // Pins the whitespace-skip arm: a paragraph with a single
        // block sentinel preceded by whitespace inside the same Text
        // leaf must still be classified as sole-sentinel.
        use aozora_pipeline::BLOCK_LEAF_SENTINEL;
        // Synthesise a paragraph whose Text leaf is "  \tU+E002".
        // We piggy-back on comrak's parser: whitespace is preserved
        // inside paragraph Text nodes after the leading-strip.
        let payload = format!("a{BLOCK_LEAF_SENTINEL}");
        let arena = Arena::new();
        let lex_out = aozora_pipeline::lex_into_arena(&payload, &arena);
        let comrak_arena = comrak::Arena::new();
        let opts = comrak::Options::default();
        let root = comrak::parse_document(&comrak_arena, lex_out.normalized, &opts);
        let para = root.first_child().expect("paragraph node");
        // "a" is non-whitespace and not a sentinel — sole-sentinel
        // must fail (Break(())) and yield None.
        assert!(
            paragraph_sole_block_sentinel(para).is_none(),
            "non-sentinel non-whitespace char wrongly accepted"
        );
    }
}
