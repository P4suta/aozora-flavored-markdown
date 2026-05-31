//! Shared primitives for both [`crate::ast_splice`] (HTML splicer)
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
//! - `flatten_registry_in_source_order` materialises the registry into
//!   a `Vec<NodeRef>` in source order via the registry's own
//!   ascending-key iterator (`iter_sorted`), since both walkers consume
//!   entries linearly and never look up by position at HTML rewrite
//!   time — the order alone is sufficient. That makes it `O(n_registry)`
//!   with no re-scan of the normalized text.
//! - `paragraph_sole_block_sentinel` walks a comrak paragraph node
//!   directly with allocation-free semantics, returning the kind of
//!   block sentinel iff the paragraph carries exactly one and no
//!   other non-whitespace content.

use core::ops::ControlFlow;

use aozora::pipeline::{
    BLOCK_CLOSE_SENTINEL, BLOCK_LEAF_SENTINEL, BLOCK_OPEN_SENTINEL, BorrowedLexOutput,
    INLINE_SENTINEL,
};
use aozora::syntax::borrowed::{AozoraNode, HeadingHint, NodeRef};
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
                    // Hold the `child.data` borrow across `visit` rather
                    // than cloning the string out. The visitor only ever
                    // sees `&str` — it cannot reach `child.data` — and
                    // every visitor on this path is read-only (the
                    // splice's tree mutation runs in a separate, later
                    // walk), so the immutable borrow is sound and the
                    // per-leaf `Cow::clone` — an owned-string deep copy
                    // on consolidated comrak text — is pure waste.
                    let flow = visit(s);
                    drop(data);
                    if flow == ControlFlow::Break(()) {
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

/// Walk a comrak paragraph node and return `Some(kind)` iff its
/// body, taken across all `Text`-node descendants, contains exactly
/// one block-sentinel codepoint and otherwise consists only of ASCII
/// whitespace, AND the paragraph has no non-`Text` descendants
/// (which would imply embedded inline structure incompatible with a
/// sole-sentinel paragraph). Allocation-free.
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
    // The registry is already keyed by normalized position, so its
    // ascending-key iterator yields exactly the sentinel nodes in source
    // order — O(n_registry). The previous implementation re-scanned the
    // whole `normalized` text (`char_indices`) and did a binary-search
    // `node_at` per sentinel — O(n_norm × log n_registry) — even though
    // the registry already knew every sentinel position. Equivalence with
    // that scan is pinned by `flatten_matches_normalized_scan`.
    let mut out = Vec::with_capacity(lex_out.registry.len());
    out.extend(
        lex_out
            .registry
            .iter_sorted()
            .map(|(_pos, node_ref)| node_ref),
    );
    out
}

/// Cursor over an owned sentinel-ordered `Vec<NodeRef>`.
///
/// Both [`crate::ast_splice`] and [`crate::ir`] consume the
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

/// Single-descent paragraph profile: counts sentinel chars and
/// remembers the registry's first `HeadingHint` payload.
///
/// Both [`crate::ir`] and [`crate::ast_splice`] need this exact
/// summary to dispatch a paragraph to either heading-hint promotion
/// (Case 2) or ordinary inline processing (Case 3). Computing it here,
/// once, keeps the two walkers in lockstep without duplicating the
/// peek-and-count loop.
#[derive(Debug)]
pub(crate) struct ParaScan<'src> {
    /// Total sentinel chars in the paragraph's text descendants.
    /// Equals the number of registry entries the paragraph would
    /// consume during inline projection.
    pub(crate) total_sentinels: usize,
    /// First sentinel that the registry classifies as a heading hint.
    /// `None` if the paragraph carries no inline heading hint.
    pub(crate) first_heading_hint: Option<&'src HeadingHint<'src>>,
}

impl<'src> ParaScan<'src> {
    pub(crate) fn run<'a>(node: &'a AstNode<'a>, cursor: &SentinelCursor<'src>) -> Self {
        let mut total_sentinels = 0usize;
        let mut first_heading_hint = None;
        for_each_text_descendant(node, |text| {
            for ch in text.chars() {
                if !is_sentinel_char(ch) {
                    continue;
                }
                if first_heading_hint.is_none()
                    && let Some(NodeRef::Inline(AozoraNode::HeadingHint(h))) =
                        cursor.peek(total_sentinels)
                {
                    first_heading_hint = Some(h);
                }
                total_sentinels += 1;
            }
        });
        Self {
            total_sentinels,
            first_heading_hint,
        }
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
    fn sentinel_cursor_peeks_and_consumes_in_order() {
        // Synthesise a small slice of NodeRefs for cursor mechanics.
        use aozora::syntax::ContainerKind;
        use aozora::syntax::borrowed::AozoraNode;
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

    /// `flatten_registry_in_source_order` now reads the registry's
    /// `iter_sorted` instead of re-scanning the normalized text. Pin that
    /// the two produce the *same* source-ordered sequence — the invariant
    /// the `SentinelCursor` lockstep with `split_text_node` / `ParaScan`
    /// depends on. Checked on a sentinel-sparse (representative) and a
    /// sentinel-dense (pathological) document, since a divergence would
    /// surface in the dense case.
    #[test]
    fn flatten_matches_normalized_scan() {
        use aozora::NormalizedOffset;
        use aozora::pipeline::lex_into_arena;
        use aozora::syntax::borrowed::Arena;

        const REPRESENTATIVE: &str = "見出し\n\n本文に｜青空《あおぞら》のルビと\
            ［＃「強調」に傍点］を混ぜた段落。\n\n次の段落も｜漢字《かんじ》。";
        const PATHOLOGICAL: &str = "｜A《a》｜B《b》｜C《c》［＃「D」に傍点］｜E《e》";

        for src in [REPRESENTATIVE, PATHOLOGICAL] {
            let arena = Arena::new();
            let lex_out = lex_into_arena(src, &arena);

            // The positions the new `iter_sorted` path yields, in order.
            let via_iter_sorted: Vec<u32> =
                lex_out.registry.iter_sorted().map(|(pos, _)| pos).collect();

            // The positions the old full-normalized-scan path would yield.
            let mut via_scan: Vec<u32> = Vec::new();
            for (idx, ch) in lex_out.normalized.char_indices() {
                if !is_sentinel_char(ch) {
                    continue;
                }
                let pos = u32::try_from(idx).expect("normalized fits u32");
                if lex_out.registry.node_at(NormalizedOffset(pos)).is_some() {
                    via_scan.push(pos);
                }
            }

            assert_eq!(
                via_iter_sorted, via_scan,
                "iter_sorted order must match the normalized-scan order for {src:?}"
            );
            assert_eq!(
                flatten_registry_in_source_order(&lex_out).len(),
                lex_out.registry.len(),
                "one node per registry entry for {src:?}"
            );
        }
    }
}
