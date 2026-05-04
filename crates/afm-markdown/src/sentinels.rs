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
    if walk_text_only_descendants(node, &mut |s| {
        for ch in s.chars() {
            if matches!(ch, ' ' | '\t' | '\n' | '\r') {
                continue;
            }
            let Some(kind) = BlockSentinelKind::from_char(ch) else {
                return ControlFlow::Break;
            };
            if found.is_some() {
                return ControlFlow::Break;
            }
            found = Some(kind);
        }
        ControlFlow::Continue
    }) {
        found
    } else {
        None
    }
}

/// Result of a Text-node walk over a comrak block.
enum ControlFlow {
    Continue,
    Break,
}

/// Visit every `Text` descendant of `node` left-to-right. Returns
/// `true` iff the entire subtree consists of `Text` leaves *only*
/// (no Strong / Emph / Link / Code / etc. nodes), AND the visitor
/// closure asked to continue at every step. Used to validate the
/// "sole block sentinel paragraph" invariant cheaply.
fn walk_text_only_descendants<'a, F>(node: &'a AstNode<'a>, visit: &mut F) -> bool
where
    F: FnMut(&str) -> ControlFlow,
{
    for child in node.children() {
        let data = child.data.borrow();
        match &data.value {
            NodeValue::Text(s) => {
                let s = s.clone();
                drop(data);
                match visit(&s) {
                    ControlFlow::Continue => {}
                    ControlFlow::Break => return false,
                }
                if child.first_child().is_some() && !walk_text_only_descendants(child, visit) {
                    return false;
                }
            }
            _ => return false,
        }
    }
    true
}

/// Visit every `Text` descendant of `node` left-to-right, calling
/// `visit` on each `&str` slice. Unlike
/// [`walk_text_only_descendants`], this descends into emphasis /
/// strong / link / code subtrees and ignores their wrappers — we
/// only care about the leaf text. Used to count sentinels and peek
/// the registry for paragraph-level dispatch.
pub(crate) fn for_each_text_descendant<'a, F>(node: &'a AstNode<'a>, mut visit: F)
where
    F: FnMut(&str),
{
    visit_text_inner(node, &mut visit);
}

fn visit_text_inner<'a, F>(node: &'a AstNode<'a>, visit: &mut F)
where
    F: FnMut(&str),
{
    for child in node.children() {
        let data = child.data.borrow();
        if let NodeValue::Text(s) = &data.value {
            let s = s.clone();
            drop(data);
            visit(&s);
        } else {
            let has_descendants = child.first_child().is_some();
            drop(data);
            if has_descendants {
                visit_text_inner(child, visit);
            }
        }
    }
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

/// Cursor over a flat sentinel-ordered slice of [`NodeRef`].
///
/// Both [`crate::post_process`] and [`crate::ir`] share this cursor:
/// they track their own per-walker state (HTML buffer + container
/// kind stack vs IR tree builder + open-container stack) but agree on
/// how to consume the stream.
pub(crate) struct SentinelCursor<'a, 'src> {
    nodes: &'a [NodeRef<'src>],
    idx: usize,
}

impl<'a, 'src> SentinelCursor<'a, 'src> {
    pub(crate) fn new(nodes: &'a [NodeRef<'src>]) -> Self {
        Self { nodes, idx: 0 }
    }

    /// Peek the registry entry at `offset` past the current cursor.
    /// `peek(0)` returns the next entry that `next` would produce.
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

    /// Number of entries consumed so far. Used by streaming-mode
    /// callers (`crate::ir`'s `StreamingIrBuilder`) to thread the
    /// cursor across per-block walks.
    pub(crate) fn position(&self) -> usize {
        self.idx
    }

    /// Construct with an explicit starting cursor position.
    /// Saturating: positions past the end clamp to `nodes.len()`.
    pub(crate) fn with_position(nodes: &'a [NodeRef<'src>], pos: usize) -> Self {
        Self {
            nodes,
            idx: pos.min(nodes.len()),
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
        let entries: &[NodeRef<'static>] = &[
            NodeRef::Inline(AozoraNode::PageBreak),
            NodeRef::BlockOpen(ContainerKind::Keigakomi),
            NodeRef::BlockClose(ContainerKind::Keigakomi),
        ];
        let mut cursor = SentinelCursor::new(entries);
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
