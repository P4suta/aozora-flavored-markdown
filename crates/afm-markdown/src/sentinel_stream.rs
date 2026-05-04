//! Shared primitives for both [`crate::post_process`] (HTML splicer)
//! and [`crate::ir`] (IR builder).
//!
//! Both downstream consumers walk the same sentinel-position stream
//! produced by `aozora-pipeline`. They differ only in their emit
//! target (string buffer vs. typed tree), not in how they sequence
//! the registry. This module owns the sequencing primitives so the
//! two walkers stay in lockstep automatically.
//!
//! Design notes:
//!
//! - The fast `is_sentinel_char` check is a single subtract-and-compare
//!   on the codepoint (`ch as u32 - 0xE001 < 4`). Hotter than the
//!   `matches!` chain it replaces because every paragraph-text walk
//!   touches this predicate per char.
//! - `flatten_registry_in_source_order` materialises the registry
//!   into a `Vec<NodeRef>` keyed by source-order traversal, since
//!   both walkers consume entries linearly. The `EytzingerMap`
//!   upstream is binary-search-friendly but we never look up by
//!   position at HTML rewrite time — the order alone is sufficient.
//! - `sole_block_sentinel` walks a `&str` of paragraph-inner text
//!   without allocating; `paragraph_sole_block_sentinel` walks a
//!   comrak paragraph node directly with the same semantics, also
//!   allocation-free.

use core::ops::ControlFlow;

use aozora_pipeline::{
    BLOCK_CLOSE_SENTINEL, BLOCK_LEAF_SENTINEL, BLOCK_OPEN_SENTINEL, BorrowedLexOutput,
    INLINE_SENTINEL,
};
use aozora_spec::NormalizedOffset;
use aozora_syntax::borrowed::NodeRef;
use comrak::nodes::{AstNode, NodeValue};

/// Which paired sentinel a block-sentinel paragraph carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BlockSentinelKind {
    Leaf,
    Open,
    Close,
}

impl BlockSentinelKind {
    /// Map a char codepoint back to its sentinel kind. `None` for
    /// inline sentinel and non-sentinel chars.
    #[inline]
    pub(crate) const fn from_char(ch: char) -> Option<Self> {
        match ch {
            BLOCK_LEAF_SENTINEL => Some(Self::Leaf),
            BLOCK_OPEN_SENTINEL => Some(Self::Open),
            BLOCK_CLOSE_SENTINEL => Some(Self::Close),
            _ => None,
        }
    }
}

/// Saturating `usize → u32`. Source line / column / byte offsets
/// past `u32::MAX` only happen for files larger than `~4G`, which
/// the rest of the pipeline already declines to handle, so a
/// saturating clamp is the right answer when we have to fit a
/// `usize` into the IR / sourcepos surface.
#[inline]
#[must_use]
pub(crate) fn saturating_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// True iff `ch` is one of the four PUA sentinel codepoints
/// `U+E001..=U+E004`.
///
/// Implemented as a single subtract-and-compare. The optimiser would
/// likely fold the equivalent `matches!` chain into the same code,
/// but writing it once explicitly keeps the hot path obvious to
/// readers and lets us const-eval it where needed.
#[inline]
pub(crate) const fn is_sentinel_char(ch: char) -> bool {
    (ch as u32).wrapping_sub(INLINE_SENTINEL as u32) < 4
}

/// If `inner` consists of exactly one block-sentinel character
/// (optionally surrounded by ASCII whitespace), return its kind.
/// Inline sentinels never qualify.
pub(crate) fn sole_block_sentinel(inner: &str) -> Option<BlockSentinelKind> {
    let trimmed = inner.trim_matches(|c: char| matches!(c, ' ' | '\t' | '\n' | '\r'));
    let mut chars = trimmed.chars();
    let first = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    BlockSentinelKind::from_char(first)
}

/// How [`visit_text_leaves`] handles non-`Text` child nodes
/// (`Strong` / `Emph` / `Link` / `Code` / ...).
#[derive(Debug, Clone, Copy)]
pub(crate) enum InlineDescend {
    /// Bail out the moment a non-`Text` child is encountered. Used
    /// to validate "this paragraph is a single bare block-sentinel
    /// run" without false-positives from emphasis-wrapped content.
    StopAtNonText,
    /// Descend through emphasis / strong / link / code wrappers and
    /// keep visiting their `Text` leaves. The default for paragraph
    /// dispatch (sentinel counting, heading-hint peeking).
    DescendThrough,
}

/// Visit every `Text`-leaf descendant of `node` left-to-right.
///
/// `mode` decides what happens when the walker meets a non-`Text`
/// child (see [`InlineDescend`]). The closure is invoked once per
/// `Text` leaf with the leaf's string slice and may return
/// [`ControlFlow::Break`] to short-circuit the entire walk.
///
/// Returns `Err(())` when:
/// - `mode == StopAtNonText` and a non-`Text` child was encountered,
///   OR
/// - the closure returned `Break` at some point.
///
/// Returns `Ok(())` when the whole subtree was visited and every
/// closure invocation returned `Continue`.
///
/// `core::ops::ControlFlow<()>` is the visitor signal so callers can
/// thread their own early-bail without a bespoke enum.
pub(crate) fn visit_text_leaves<'a, F>(
    node: &'a AstNode<'a>,
    mode: InlineDescend,
    mut visit: F,
) -> Result<(), ()>
where
    F: FnMut(&str) -> ControlFlow<()>,
{
    fn recurse<'a, F>(node: &'a AstNode<'a>, mode: InlineDescend, visit: &mut F) -> Result<(), ()>
    where
        F: FnMut(&str) -> ControlFlow<()>,
    {
        for child in node.children() {
            let data = child.data.borrow();
            match &data.value {
                NodeValue::Text(s) => {
                    let s = s.clone();
                    drop(data);
                    if visit(&s) == ControlFlow::Break(()) {
                        return Err(());
                    }
                    // A `Text` node can in principle have children
                    // under non-pathological comrak inputs (emphasis
                    // splits etc.). Recurse through them too.
                    if child.first_child().is_some() {
                        recurse(child, mode, visit)?;
                    }
                }
                _ => match mode {
                    InlineDescend::StopAtNonText => return Err(()),
                    InlineDescend::DescendThrough => {
                        let has_descendants = child.first_child().is_some();
                        drop(data);
                        if has_descendants {
                            recurse(child, mode, visit)?;
                        }
                    }
                },
            }
        }
        Ok(())
    }
    recurse(node, mode, &mut visit)
}

/// Allocation-free analogue of [`sole_block_sentinel`] that walks a
/// comrak paragraph node directly.
///
/// Returns `Some(kind)` iff the paragraph's body, taken across all
/// `Text`-node descendants, contains exactly one block-sentinel
/// codepoint and otherwise consists only of ASCII whitespace, AND
/// the paragraph has no non-`Text` descendants (which would imply
/// embedded inline structure incompatible with a sole-sentinel
/// paragraph).
pub(crate) fn paragraph_sole_block_sentinel<'a>(
    node: &'a AstNode<'a>,
) -> Option<BlockSentinelKind> {
    let mut found: Option<BlockSentinelKind> = None;
    let walk_ok = visit_text_leaves(node, InlineDescend::StopAtNonText, |s| {
        for ch in s.chars() {
            if matches!(ch, ' ' | '\t' | '\n' | '\r') {
                continue;
            }
            let Some(kind) = BlockSentinelKind::from_char(ch) else {
                return ControlFlow::Break(());
            };
            if found.is_some() {
                return ControlFlow::Break(());
            }
            found = Some(kind);
        }
        ControlFlow::Continue(())
    })
    .is_ok();
    walk_ok.then_some(()).and(found)
}

/// Visit every `Text` descendant of `node` left-to-right, descending
/// through emphasis / strong / link / code wrappers. Unlike the
/// general [`visit_text_leaves`] this never bails — used for the
/// paragraph-level sentinel count + heading-hint peek where every
/// leaf must be observed.
pub(crate) fn for_each_text_descendant<'a, F>(node: &'a AstNode<'a>, mut visit: F)
where
    F: FnMut(&str),
{
    // `DescendThrough` + `Continue` can never short-circuit, so the
    // returned Result is structurally always `Ok(())`; we discard it.
    let _result = visit_text_leaves(node, InlineDescend::DescendThrough, |s| {
        visit(s);
        ControlFlow::Continue(())
    });
}

/// Walk `lex_out.normalized` byte-by-byte; for every PUA sentinel,
/// query the registry and append the resulting [`NodeRef`] to a
/// freshly-allocated `Vec` in source order.
///
/// Returns an empty vec when the registry is empty (the typical
/// branch when `Options::aozora_enabled` is `false`).
pub(crate) fn flatten_registry_in_source_order<'a>(
    lex_out: &BorrowedLexOutput<'a>,
) -> Vec<NodeRef<'a>> {
    if lex_out.registry.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(lex_out.registry.len());
    for (idx, ch) in lex_out.normalized.char_indices() {
        if !is_sentinel_char(ch) {
            continue;
        }
        let pos = u32::try_from(idx).expect("normalized text fits u32 (Phase 0 cap)");
        if let Some(node_ref) = lex_out.registry.node_at(NormalizedOffset(pos)) {
            out.push(node_ref);
        }
    }
    out
}

/// Cursor over an owned sentinel-ordered `Vec<NodeRef>`.
///
/// Both [`crate::post_process`] and [`crate::ir`] consume the
/// registry by materialising it into a `Vec` once, then walking it
/// linearly. The cursor owns that `Vec` so callers don't have to
/// thread a separate slice lifetime through every walker — a single
/// `'src` (the borrowed-AST payload lifetime) is enough.
#[derive(Debug)]
pub(crate) struct SentinelCursor<'src> {
    nodes: Vec<NodeRef<'src>>,
    idx: usize,
}

impl<'src> SentinelCursor<'src> {
    /// Materialise the registry into a fresh cursor. Empty `lex_out`
    /// produces a cursor with no entries; consumers degrade to
    /// markdown-only behaviour.
    pub(crate) fn from_lex_out(lex_out: Option<&BorrowedLexOutput<'src>>) -> Self {
        Self {
            nodes: lex_out
                .map(flatten_registry_in_source_order)
                .unwrap_or_default(),
            idx: 0,
        }
    }

    /// Construct directly from a `Vec` of registry entries (used
    /// by tests and by the streaming builder which owns the `Vec`).
    pub(crate) fn from_nodes(nodes: Vec<NodeRef<'src>>) -> Self {
        Self { nodes, idx: 0 }
    }

    /// Peek the registry entry at `offset` past the current cursor.
    /// `peek(0)` returns the next entry that [`Self::next`] would
    /// produce.
    pub(crate) fn peek(&self, offset: usize) -> Option<NodeRef<'src>> {
        self.nodes.get(self.idx + offset).copied()
    }

    /// Consume and return the next entry, advancing the cursor.
    pub(crate) fn next(&mut self) -> Option<NodeRef<'src>> {
        let n = self.nodes.get(self.idx).copied();
        if n.is_some() {
            self.idx += 1;
        }
        n
    }

    /// Saturating advance by `n` entries.
    pub(crate) fn advance(&mut self, n: usize) {
        self.idx = self.idx.saturating_add(n).min(self.nodes.len());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_sentinel_char_recognises_all_four() {
        for ch in [
            INLINE_SENTINEL,
            BLOCK_LEAF_SENTINEL,
            BLOCK_OPEN_SENTINEL,
            BLOCK_CLOSE_SENTINEL,
        ] {
            assert!(is_sentinel_char(ch), "{ch:?} should be a sentinel");
        }
    }

    #[test]
    fn is_sentinel_char_rejects_neighbours() {
        // Codepoints adjacent to the sentinel range must NOT match.
        assert!(!is_sentinel_char('\u{E000}'));
        assert!(!is_sentinel_char('\u{E005}'));
        assert!(!is_sentinel_char('a'));
        assert!(!is_sentinel_char('\0'));
    }

    #[test]
    fn block_sentinel_kind_from_char_round_trips() {
        assert_eq!(
            BlockSentinelKind::from_char(BLOCK_LEAF_SENTINEL),
            Some(BlockSentinelKind::Leaf)
        );
        assert_eq!(
            BlockSentinelKind::from_char(BLOCK_OPEN_SENTINEL),
            Some(BlockSentinelKind::Open)
        );
        assert_eq!(
            BlockSentinelKind::from_char(BLOCK_CLOSE_SENTINEL),
            Some(BlockSentinelKind::Close)
        );
        // Inline does NOT count as a block sentinel.
        assert!(BlockSentinelKind::from_char(INLINE_SENTINEL).is_none());
        assert!(BlockSentinelKind::from_char('a').is_none());
    }

    #[test]
    fn sole_block_sentinel_accepts_block_with_whitespace_around() {
        assert_eq!(
            sole_block_sentinel(&format!("\n{BLOCK_LEAF_SENTINEL}\n")),
            Some(BlockSentinelKind::Leaf)
        );
    }

    #[test]
    fn sole_block_sentinel_rejects_inline() {
        assert!(sole_block_sentinel(&format!("{INLINE_SENTINEL}")).is_none());
    }

    #[test]
    fn sole_block_sentinel_rejects_multiple() {
        let s = format!("{BLOCK_LEAF_SENTINEL}{BLOCK_OPEN_SENTINEL}");
        assert!(sole_block_sentinel(&s).is_none());
    }

    #[test]
    fn sentinel_cursor_peeks_and_consumes_in_order() {
        // Synthesise a small slice of NodeRefs for cursor mechanics.
        use aozora_syntax::ContainerKind;
        use aozora_syntax::borrowed::AozoraNode;
        let entries: Vec<NodeRef<'static>> = vec![
            NodeRef::Inline(AozoraNode::PageBreak),
            NodeRef::BlockOpen(ContainerKind::Keigakomi),
            NodeRef::BlockClose(ContainerKind::Keigakomi),
        ];
        let mut cursor = SentinelCursor::from_nodes(entries);
        assert!(matches!(
            cursor.peek(0),
            Some(NodeRef::Inline(AozoraNode::PageBreak))
        ));
        assert!(matches!(
            cursor.peek(2),
            Some(NodeRef::BlockClose(ContainerKind::Keigakomi))
        ));
        assert!(cursor.peek(3).is_none());
        let _ = cursor.next();
        assert!(matches!(
            cursor.next(),
            Some(NodeRef::BlockOpen(ContainerKind::Keigakomi))
        ));
        cursor.advance(99); // saturating
        assert!(cursor.next().is_none());
    }
}
