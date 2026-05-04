//! Intermediate representation produced by [`crate::render_to_ir`].
//!
//! The shape mirrors the TypeScript `IRDocument` consumed by
//! `afm-obsidian/src/ir/types.ts` (and validated in
//! `afm-obsidian/src/ir/from-wasm.ts`). Keeping the names and field
//! ordering aligned across the FFI boundary makes the
//! `serde-wasm-bindgen` round-trip a pass-through, no shape adapters
//! needed.
//!
//! # Coverage
//!
//! - **Markdown side**: paragraphs, headings, lists, blockquotes,
//!   fenced code, tables, thematic breaks. Inline runs preserve
//!   `Strong`, `Emphasis`, `Link`, `Code`, `LineBreak`, and verbatim
//!   `Text`.
//! - **Aozora side**: `Ruby` / `DoubleRuby` / `Bouten` / `Tcy` /
//!   `Gaiji` / `Annotation` (inline) and `Container` / `PageBreak` /
//!   `SectionBreak` (block). Heading hints
//!   (`［＃「X」は大見出し］`) promote their host paragraph to
//!   `IrBlock::Heading` directly, mirroring [`crate::post_process`].
//!
//! # Architecture
//!
//! The walker is built from three small primitives:
//!
//! 1. [`crate::sentinels::SentinelCursor`] — the shared registry-stream
//!    cursor. The HTML splicer ([`crate::post_process`]) and this
//!    builder both consume the same source-order sequence of
//!    `NodeRef` entries; the cursor abstraction keeps them in
//!    lockstep.
//! 2. [`ParaScan`] — single-descent paragraph profile. One walk per
//!    paragraph computes both the sole-block-sentinel test and the
//!    heading-hint lookahead at once, eliminating the two-scan
//!    redundancy that a naive translation of the HTML splicer would
//!    have.
//! 3. [`OpenContainer`] — the per-walker container stack. Where the
//!    HTML splicer can stream open/close tags into a string buffer,
//!    the IR demands a tree, so each open container collects
//!    `IrBlock`s into its own `Vec` until the matching close arrives.
//!    Move semantics (no `clone`) carry the children into the closed
//!    `IrBlock::Container`.

use aozora_encoding::gaiji::Resolved;
use aozora_pipeline::BorrowedLexOutput;
use aozora_syntax::borrowed::{
    Annotation as AozoraAnnotation, AozoraNode, Bouten as AozoraBouten, Content,
    DoubleRuby as AozoraDoubleRuby, Gaiji as AozoraGaiji, HeadingHint, NodeRef, Ruby as AozoraRuby,
    Segment, TateChuYoko,
};
use aozora_syntax::{AnnotationKind, BoutenKind, BoutenPosition, ContainerKind, SectionKind};
use comrak::nodes::{
    AstNode, ListType, NodeHeading, NodeList, NodeValue, Sourcepos, TableAlignment,
};
use serde::Serialize;

use crate::sentinels::{
    BlockSentinelKind, SentinelCursor, flatten_registry_in_source_order, for_each_text_descendant,
    is_sentinel_char, paragraph_sole_block_sentinel,
};

/// Saturating `usize → u32`. Source line/column overflow requires
/// `~4G`-line files, so saturating to `u32::MAX` is safe.
fn to_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IrDocument {
    pub blocks: Vec<IrBlock>,
    pub diagnostics: Vec<IrDiagnostic>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum IrBlock {
    Paragraph {
        children: Vec<IrInline>,
        #[serde(skip_serializing_if = "Option::is_none")]
        source_line: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        range: Option<Range>,
    },
    Heading {
        level: u8,
        children: Vec<IrInline>,
        #[serde(skip_serializing_if = "Option::is_none")]
        source_line: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        range: Option<Range>,
    },
    Blockquote {
        children: Vec<Self>,
        #[serde(skip_serializing_if = "Option::is_none")]
        source_line: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        range: Option<Range>,
    },
    List {
        ordered: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        start: Option<u32>,
        items: Vec<IrListItem>,
        #[serde(skip_serializing_if = "Option::is_none")]
        source_line: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        range: Option<Range>,
    },
    CodeBlock {
        #[serde(skip_serializing_if = "Option::is_none")]
        lang: Option<String>,
        value: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        source_line: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        range: Option<Range>,
    },
    ThematicBreak {
        #[serde(skip_serializing_if = "Option::is_none")]
        source_line: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        range: Option<Range>,
    },
    Table {
        header: IrTableRow,
        rows: Vec<IrTableRow>,
        align: Vec<IrTableAlign>,
        #[serde(skip_serializing_if = "Option::is_none")]
        source_line: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        range: Option<Range>,
    },
    // ----- Aozora-specific block variants -----
    /// Paired-container wrapper. `subtype` is one of `"indent"`,
    /// `"alignEnd"`, `"keigakomi"`, `"warichu"`. `indent_level` is set
    /// to `Some(n)` for `"indent"` (字下げ amount) and `"alignEnd"`
    /// (地上げ offset); `None` otherwise.
    Container {
        subtype: String,
        children: Vec<Self>,
        #[serde(skip_serializing_if = "Option::is_none")]
        indent_level: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        source_line: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        range: Option<Range>,
    },
    PageBreak {
        #[serde(skip_serializing_if = "Option::is_none")]
        source_line: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        range: Option<Range>,
    },
    /// `［＃改丁／改段／改見開き］`. `subtype` is one of `"choho"`,
    /// `"dan"`, `"spread"` (camelCase tags matching upstream
    /// [`SectionKind`]). `［＃改ページ］` is its own block — see
    /// [`IrBlock::PageBreak`].
    SectionBreak {
        subtype: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        source_line: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        range: Option<Range>,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct IrTableRow {
    pub cells: Vec<Vec<IrInline>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<Range>,
}

#[derive(Debug, Clone, Serialize)]
pub struct IrListItem {
    pub children: Vec<IrBlock>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<Range>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum IrTableAlign {
    Left,
    Center,
    Right,
    Default,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum IrInline {
    Text {
        value: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        range: Option<Range>,
    },
    Code {
        value: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        range: Option<Range>,
    },
    Strong {
        children: Vec<Self>,
        #[serde(skip_serializing_if = "Option::is_none")]
        range: Option<Range>,
    },
    Emphasis {
        children: Vec<Self>,
        #[serde(skip_serializing_if = "Option::is_none")]
        range: Option<Range>,
    },
    Link {
        href: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        children: Vec<Self>,
        #[serde(skip_serializing_if = "Option::is_none")]
        range: Option<Range>,
    },
    /// CommonMark image. `alt` carries the alt-text inlines exactly
    /// as comrak parses them (typically a single `Text`). `url` is
    /// the image source; `title` is the optional `"…"` argument.
    Image {
        url: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        alt: Vec<Self>,
        #[serde(skip_serializing_if = "Option::is_none")]
        range: Option<Range>,
    },
    LineBreak {
        hard: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        range: Option<Range>,
    },
    // ----- Aozora-specific variants (mirror TS IRInline) -----
    /// Furigana. `reading` is the flattened reading text;
    /// `explicit` is `true` when the source used the explicit
    /// `｜base《reading》` opener.
    Ruby {
        base: Vec<Self>,
        reading: String,
        explicit: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        range: Option<Range>,
    },
    /// `《《…》》` double-bracket bouten. Upstream's `DoubleRuby`
    /// carries a single `content` payload — that payload becomes
    /// `base` here. The shape is intentionally minimal: any future
    /// upstream addition (e.g., explicit ring-style metadata) lands
    /// as a new optional field rather than re-using empty strings as
    /// placeholders.
    DoubleRuby {
        base: Vec<Self>,
        #[serde(skip_serializing_if = "Option::is_none")]
        range: Option<Range>,
    },
    /// Emphasis dots / sidelines. `style` is one of `"goma"`,
    /// `"whiteSesame"`, `"circle"`, `"whiteCircle"`, `"doubleCircle"`,
    /// `"janome"`, `"cross"`, `"whiteTriangle"`, `"wavyLine"`,
    /// `"underLine"`, `"doubleUnderLine"`. `position` is `"right"` or
    /// `"left"`.
    Bouten {
        children: Vec<Self>,
        style: String,
        position: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        range: Option<Range>,
    },
    Gaiji {
        #[serde(skip_serializing_if = "Option::is_none")]
        codepoint: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        fallback_text: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        range: Option<Range>,
    },
    Tcy {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        range: Option<Range>,
    },
    /// Generic annotation. `payload` is the raw bytes between
    /// `［＃` and `］`. `resolved` carries the [`AnnotationKind`]
    /// camelCase tag (`"asIs"`, `"textualNote"`, `"invalidRubySpan"`,
    /// `"warichuOpen"`, `"warichuClose"`) when the upstream lexer
    /// classified the annotation; `None` for `Unknown`.
    Annotation {
        payload: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        resolved: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        range: Option<Range>,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct IrDiagnostic {
    pub level: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<Range>,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Range {
    pub from: u32,
    pub to: u32,
}

// ===================================================================
// Walker entry points
// ===================================================================

/// Walk a comrak AST root and project it to [`IrDocument`].
///
/// `lex_out` carries the borrowed-AST registry. When `Some`, every
/// PUA sentinel in the comrak text is projected to its matching
/// [`IrBlock`] / [`IrInline`] variant; when `None`, the walker
/// degrades to markdown-only behaviour (used by
/// `Options::aozora_enabled = false`).
pub(crate) fn build_ir<'a>(
    root: &'a AstNode<'a>,
    lex_out: Option<&BorrowedLexOutput<'a>>,
) -> IrDocument {
    let nodes = lex_out
        .map(flatten_registry_in_source_order)
        .unwrap_or_default();
    let mut walker = IrWalker::new(nodes.as_slice());
    walker.walk_root(root);
    IrDocument {
        blocks: walker.finish(),
        diagnostics: Vec::new(),
    }
}

/// Stateful per-block IR builder for streaming mode.
///
/// Materialises the registry once at construction time and threads a
/// shared cursor across successive `walk_block` calls so multi-block
/// inputs preserve the registry's source order. The cursor lives in
/// this struct (not in the walker) so individual `walk_block` calls
/// can be issued lazily — afm-obsidian's chunked-cancellation path
/// (ADR-0009) uses this to checkpoint between blocks.
///
/// Container open/close paragraphs that span multiple top-level
/// blocks emit fragmented `IrBlock::Container` blocks: the open
/// pushes onto a stack that drains at the next `walk_block` boundary
/// (so each block is internally consistent in nesting). Whole-doc
/// `build_ir` remains the canonical path for cross-block nesting.
#[derive(Debug)]
pub struct StreamingIrBuilder<'src> {
    nodes: Vec<NodeRef<'src>>,
    cursor_idx: usize,
}

impl<'src> StreamingIrBuilder<'src> {
    /// Materialise the registry once. `None` produces an empty
    /// builder that degrades to markdown-only projection.
    #[must_use]
    pub fn new(lex_out: Option<&BorrowedLexOutput<'src>>) -> Self {
        Self {
            nodes: lex_out
                .map(flatten_registry_in_source_order)
                .unwrap_or_default(),
            cursor_idx: 0,
        }
    }

    /// Walk a single comrak block, advancing the shared cursor.
    /// Streaming-mode containers fragment per-block; for whole-doc
    /// nesting use [`build_ir`].
    pub fn walk_block<'a>(&mut self, node: &'a AstNode<'a>) -> Vec<IrBlock> {
        let mut walker = IrWalker::with_cursor_idx(self.nodes.as_slice(), self.cursor_idx);
        walker.walk_top(node);
        let next_idx = walker.cursor.position();
        let blocks = walker.finish();
        self.cursor_idx = next_idx;
        blocks
    }
}

// ===================================================================
// Walker
// ===================================================================

/// Tree builder that consumes comrak nodes plus a sentinel cursor and
/// emits `IrBlock`s into a stack-balanced container hierarchy.
///
/// The state mirrors [`crate::post_process`]'s `SpliceState` for the
/// HTML side: same cursor, same balanced-container model, same
/// orphan-close drain at end-of-document. They differ only in the
/// emit target (string buffer vs. tree of `Vec<IrBlock>`).
///
/// Lifetimes:
///
/// - `'c` is the lifetime of the registry slice the walker borrows
///   (typically a local `Vec<NodeRef<'src>>` materialised at the call
///   site).
/// - `'src` is the arena/source lifetime that every borrowed
///   [`AozoraNode`] payload references.
struct IrWalker<'c, 'src> {
    cursor: SentinelCursor<'c, 'src>,
    /// Document-level blocks gathered so far. When a container is
    /// open, new blocks go onto its top-of-stack `children` instead.
    top: Vec<IrBlock>,
    /// Stack of currently-open paired containers. Each frame owns the
    /// blocks gathered between its open and (eventual) close marker.
    open: Vec<OpenContainer>,
}

struct OpenContainer {
    kind: ContainerKind,
    source_line: Option<u32>,
    children: Vec<IrBlock>,
}

impl<'c, 'src> IrWalker<'c, 'src> {
    fn new(nodes: &'c [NodeRef<'src>]) -> Self {
        Self {
            cursor: SentinelCursor::new(nodes),
            top: Vec::new(),
            open: Vec::new(),
        }
    }

    /// Construct a walker that resumes from a given cursor index in
    /// `nodes`. Used by [`StreamingIrBuilder`] to thread cursor state
    /// across per-block walks.
    fn with_cursor_idx(nodes: &'c [NodeRef<'src>], idx: usize) -> Self {
        Self {
            cursor: SentinelCursor::with_position(nodes, idx),
            top: Vec::new(),
            open: Vec::new(),
        }
    }

    /// Consume the walker, draining any unclosed containers (mirror of
    /// the HTML splicer's end-of-document orphan-close pass).
    fn finish(mut self) -> Vec<IrBlock> {
        while let Some(open) = self.open.pop() {
            let block = open.into_block();
            place_in(&mut self.open, &mut self.top, block);
        }
        self.top
    }

    fn walk_root<'a>(&mut self, root: &'a AstNode<'a>) {
        for child in root.children() {
            self.walk_top(child);
        }
    }

    fn walk_top<'a>(&mut self, node: &'a AstNode<'a>) {
        let (source_line, is_paragraph) = top_metadata(node);
        if is_paragraph && let Some(action) = self.classify_paragraph(node) {
            self.dispatch_paragraph(action, source_line);
            return;
        }
        if let Some(block) = self.walk_block(node, true) {
            place_in(&mut self.open, &mut self.top, block);
        }
    }

    /// Run a single descent over `node`'s text descendants, returning
    /// the most specific paragraph action (sole block sentinel or
    /// heading hint promotion) supported by the registry lookahead.
    fn classify_paragraph<'a>(&self, node: &'a AstNode<'a>) -> Option<ParagraphAction<'src>> {
        if let Some(kind) = paragraph_sole_block_sentinel(node) {
            return Some(ParagraphAction::BlockSentinel(kind));
        }
        let scan = ParaScan::run(node, &self.cursor);
        if let Some(hint) = scan.first_heading_hint {
            return Some(ParagraphAction::HeadingHint {
                hint,
                sentinels_to_consume: scan.total_sentinels,
            });
        }
        None
    }

    fn dispatch_paragraph(&mut self, action: ParagraphAction<'src>, source_line: u32) {
        match action {
            ParagraphAction::BlockSentinel(kind) => self.handle_block_sentinel(kind, source_line),
            ParagraphAction::HeadingHint {
                hint,
                sentinels_to_consume,
            } => self.handle_heading_hint(hint, sentinels_to_consume, source_line),
        }
    }

    fn handle_block_sentinel(&mut self, kind: BlockSentinelKind, source_line: u32) {
        let Some(node_ref) = self.cursor.next() else {
            return;
        };
        match (kind, node_ref) {
            (BlockSentinelKind::Leaf, NodeRef::BlockLeaf(leaf)) => {
                if let Some(block) = project_block_leaf(leaf, source_line) {
                    place_in(&mut self.open, &mut self.top, block);
                }
            }
            (BlockSentinelKind::Open, NodeRef::BlockOpen(ck)) => {
                self.open.push(OpenContainer {
                    kind: ck,
                    source_line: Some(source_line),
                    children: Vec::new(),
                });
            }
            (BlockSentinelKind::Close, NodeRef::BlockClose(_)) => {
                if let Some(open) = self.open.pop() {
                    let block = open.into_block();
                    place_in(&mut self.open, &mut self.top, block);
                }
                // Orphan close: silently dropped, in lockstep with
                // `splice_aozora_html`'s defensive guard.
            }
            _ => {}
        }
    }

    fn handle_heading_hint(
        &mut self,
        hint: &'src HeadingHint<'src>,
        sentinels_to_consume: usize,
        source_line: u32,
    ) {
        self.cursor.advance(sentinels_to_consume);
        let block = IrBlock::Heading {
            level: hint.level.clamp(1, 6),
            children: vec![IrInline::Text {
                value: hint.target.as_str().to_owned(),
                range: None,
            }],
            source_line: Some(source_line),
            range: None,
        };
        place_in(&mut self.open, &mut self.top, block);
    }

    fn walk_block<'a>(&mut self, node: &'a AstNode<'a>, top_level: bool) -> Option<IrBlock> {
        let data = node.data.borrow();
        let source_line = top_level.then(|| to_u32(data.sourcepos.start.line).max(1));
        let range = sourcepos_to_range(&data.sourcepos);
        match &data.value {
            NodeValue::Paragraph => {
                drop(data);
                Some(IrBlock::Paragraph {
                    children: self.collect_inlines(node),
                    source_line,
                    range,
                })
            }
            NodeValue::Heading(NodeHeading { level, .. }) => {
                let level = (*level).clamp(1, 6);
                drop(data);
                Some(IrBlock::Heading {
                    level,
                    children: self.collect_inlines(node),
                    source_line,
                    range,
                })
            }
            NodeValue::BlockQuote => {
                drop(data);
                Some(IrBlock::Blockquote {
                    children: self.collect_blocks(node),
                    source_line,
                    range,
                })
            }
            NodeValue::List(NodeList {
                list_type, start, ..
            }) => {
                let ordered = matches!(list_type, ListType::Ordered);
                let start = (*start > 1).then(|| to_u32(*start));
                drop(data);
                Some(IrBlock::List {
                    ordered,
                    start,
                    items: self.collect_list_items(node),
                    source_line,
                    range,
                })
            }
            NodeValue::CodeBlock(code) => {
                let lang = (!code.info.is_empty()).then(|| code.info.clone());
                let value = code.literal.clone();
                drop(data);
                Some(IrBlock::CodeBlock {
                    lang,
                    value,
                    source_line,
                    range,
                })
            }
            NodeValue::ThematicBreak => {
                drop(data);
                Some(IrBlock::ThematicBreak { source_line, range })
            }
            NodeValue::Table(table) => {
                let aligns: Vec<IrTableAlign> =
                    table.alignments.iter().copied().map(table_align).collect();
                drop(data);
                Some(self.walk_table(
                    node,
                    TableMeta {
                        align: aligns,
                        source_line,
                        range,
                    },
                ))
            }
            // List items, table rows, and table cells are handled by
            // their parents. Other unhandled block kinds (definition
            // list, footnote refs, etc.) drop from the IR — the HTML
            // still has them.
            _ => None,
        }
    }

    fn walk_table<'a>(&mut self, node: &'a AstNode<'a>, meta: TableMeta) -> IrBlock {
        let mut rows: Vec<IrTableRow> = Vec::new();
        for child in node.children() {
            rows.push(self.collect_table_row(child));
        }
        let header = rows.first().cloned().unwrap_or(IrTableRow {
            cells: Vec::new(),
            range: None,
        });
        let body = if rows.is_empty() {
            Vec::new()
        } else {
            rows[1..].to_vec()
        };
        IrBlock::Table {
            header,
            rows: body,
            align: meta.align,
            source_line: meta.source_line,
            range: meta.range,
        }
    }

    fn collect_blocks<'a>(&mut self, node: &'a AstNode<'a>) -> Vec<IrBlock> {
        let mut out = Vec::new();
        for child in node.children() {
            if let Some(block) = self.walk_block(child, false) {
                out.push(block);
            }
        }
        out
    }

    fn collect_list_items<'a>(&mut self, node: &'a AstNode<'a>) -> Vec<IrListItem> {
        let mut out = Vec::new();
        for child in node.children() {
            let data = child.data.borrow();
            let is_item = matches!(data.value, NodeValue::Item(_));
            let range = sourcepos_to_range(&data.sourcepos);
            drop(data);
            if !is_item {
                continue;
            }
            out.push(IrListItem {
                children: self.collect_blocks(child),
                range,
            });
        }
        out
    }

    fn collect_table_row<'a>(&mut self, row: &'a AstNode<'a>) -> IrTableRow {
        let data = row.data.borrow();
        let range = sourcepos_to_range(&data.sourcepos);
        drop(data);
        let mut cells = Vec::new();
        for cell in row.children() {
            cells.push(self.collect_inlines(cell));
        }
        IrTableRow { cells, range }
    }

    fn collect_inlines<'a>(&mut self, node: &'a AstNode<'a>) -> Vec<IrInline> {
        let mut out = Vec::new();
        for child in node.children() {
            self.emit_inline(child, &mut out);
        }
        out
    }

    fn emit_inline<'a>(&mut self, node: &'a AstNode<'a>, out: &mut Vec<IrInline>) {
        let data = node.data.borrow();
        let range = sourcepos_to_range(&data.sourcepos);
        match &data.value {
            NodeValue::Text(s) => {
                let s = s.clone();
                drop(data);
                self.project_text_with_sentinels(&s, range, out);
            }
            NodeValue::Code(c) => {
                let value = c.literal.clone();
                drop(data);
                out.push(IrInline::Code { value, range });
            }
            NodeValue::Strong => {
                drop(data);
                out.push(IrInline::Strong {
                    children: self.collect_inlines(node),
                    range,
                });
            }
            NodeValue::Emph => {
                drop(data);
                out.push(IrInline::Emphasis {
                    children: self.collect_inlines(node),
                    range,
                });
            }
            NodeValue::Link(link) => {
                let href = link.url.clone();
                let title = (!link.title.is_empty()).then(|| link.title.clone());
                drop(data);
                out.push(IrInline::Link {
                    href,
                    title,
                    children: self.collect_inlines(node),
                    range,
                });
            }
            NodeValue::Image(image) => {
                let url = image.url.clone();
                let title = (!image.title.is_empty()).then(|| image.title.clone());
                drop(data);
                out.push(IrInline::Image {
                    url,
                    title,
                    alt: self.collect_inlines(node),
                    range,
                });
            }
            NodeValue::SoftBreak => {
                drop(data);
                out.push(IrInline::LineBreak { hard: false, range });
            }
            NodeValue::LineBreak => {
                drop(data);
                out.push(IrInline::LineBreak { hard: true, range });
            }
            // Image, footnote refs, raw HTML, etc. drop quietly.
            _ => {}
        }
    }

    fn project_text_with_sentinels(
        &mut self,
        text: &str,
        range: Option<Range>,
        out: &mut Vec<IrInline>,
    ) {
        // Fast path: no sentinels in this text run.
        if !text.chars().any(is_sentinel_char) {
            if !text.is_empty() {
                out.push(IrInline::Text {
                    value: text.to_owned(),
                    range,
                });
            }
            return;
        }
        let mut cursor = 0;
        for (idx, ch) in text.char_indices() {
            if !is_sentinel_char(ch) {
                continue;
            }
            let head = &text[cursor..idx];
            if !head.is_empty() {
                out.push(IrInline::Text {
                    value: head.to_owned(),
                    range,
                });
            }
            cursor = idx + ch.len_utf8();
            let Some(node_ref) = self.cursor.next() else {
                continue;
            };
            // Block sentinels surviving into an inline context (e.g.
            // raw text inside a fenced code block) drop silently —
            // matches `crate::post_process::splice_inline_pass`.
            if let NodeRef::Inline(aozora) = node_ref
                && let Some(inline) = project_inline(aozora)
            {
                out.push(inline);
            }
        }
        let tail = &text[cursor..];
        if !tail.is_empty() {
            out.push(IrInline::Text {
                value: tail.to_owned(),
                range,
            });
        }
    }
}

/// Push `block` onto the top-of-stack open container's children, or
/// onto the document's top-level blocks if no container is open.
fn place_in(open: &mut [OpenContainer], top: &mut Vec<IrBlock>, block: IrBlock) {
    if let Some(frame) = open.last_mut() {
        frame.children.push(block);
    } else {
        top.push(block);
    }
}

impl OpenContainer {
    fn into_block(self) -> IrBlock {
        IrBlock::Container {
            subtype: container_subtype(self.kind).to_owned(),
            children: self.children,
            indent_level: container_indent_level(self.kind),
            source_line: self.source_line,
            range: None,
        }
    }
}

struct TableMeta {
    align: Vec<IrTableAlign>,
    source_line: Option<u32>,
    range: Option<Range>,
}

#[derive(Debug, Clone, Copy)]
enum ParagraphAction<'src> {
    BlockSentinel(BlockSentinelKind),
    HeadingHint {
        hint: &'src HeadingHint<'src>,
        sentinels_to_consume: usize,
    },
}

fn top_metadata<'a>(node: &'a AstNode<'a>) -> (u32, bool) {
    let data = node.data.borrow();
    let line = u32::try_from(data.sourcepos.start.line)
        .unwrap_or(u32::MAX)
        .max(1);
    let is_para = matches!(data.value, NodeValue::Paragraph);
    (line, is_para)
}

// ===================================================================
// Single-descent paragraph profile.
// ===================================================================

/// Collected paragraph properties. The walker computes this in one
/// pass over the paragraph's text descendants and dispatches off the
/// result.
struct ParaScan<'src> {
    /// Total sentinel chars in the paragraph's text descendants.
    /// Equals the number of registry entries the paragraph would
    /// consume during inline projection.
    total_sentinels: usize,
    /// First sentinel that the registry classifies as a heading hint.
    /// `None` if the paragraph carries no inline heading hint.
    first_heading_hint: Option<&'src HeadingHint<'src>>,
}

impl<'src> ParaScan<'src> {
    fn run<'a>(node: &'a AstNode<'a>, cursor: &SentinelCursor<'_, 'src>) -> Self {
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

// ===================================================================
// AozoraNode → IR projection.
// ===================================================================

fn project_inline(node: AozoraNode<'_>) -> Option<IrInline> {
    match node {
        AozoraNode::Ruby(r) => Some(project_ruby(r)),
        AozoraNode::DoubleRuby(d) => Some(project_double_ruby(d)),
        AozoraNode::Bouten(b) => Some(project_bouten(b)),
        AozoraNode::TateChuYoko(t) => Some(project_tcy(t)),
        AozoraNode::Gaiji(g) => Some(project_gaiji(g)),
        AozoraNode::Annotation(a) => Some(project_annotation(a)),
        // HeadingHint is consumed at the paragraph level, never inline.
        // Other variants (`Indent` leaf, `AlignEnd` leaf, `Warichu`,
        // `Sashie`, `Kaeriten`, `AozoraHeading`, `Keigakomi`) exist as
        // block markers in upstream and don't have a v0.2 inline
        // projection. They appear in the HTML but drop from the IR.
        _ => None,
    }
}

fn project_block_leaf(node: AozoraNode<'_>, source_line: u32) -> Option<IrBlock> {
    match node {
        AozoraNode::PageBreak => Some(IrBlock::PageBreak {
            source_line: Some(source_line),
            range: None,
        }),
        AozoraNode::SectionBreak(kind) => Some(IrBlock::SectionBreak {
            subtype: section_kind_subtype(kind).to_owned(),
            source_line: Some(source_line),
            range: None,
        }),
        // Other block-leaf variants (`Sashie`, `AozoraHeading`, …)
        // have no v0.2 IR projection. The HTML still carries them.
        _ => None,
    }
}

fn project_ruby(r: &AozoraRuby<'_>) -> IrInline {
    IrInline::Ruby {
        base: project_content_inlines(r.base.get()),
        reading: content_to_string(r.reading.get()),
        explicit: r.delim_explicit,
        range: None,
    }
}

fn project_double_ruby(d: &AozoraDoubleRuby<'_>) -> IrInline {
    IrInline::DoubleRuby {
        base: project_content_inlines(d.content.get()),
        range: None,
    }
}

fn project_bouten(b: &AozoraBouten<'_>) -> IrInline {
    IrInline::Bouten {
        children: project_content_inlines(b.target.get()),
        style: bouten_kind_str(b.kind).to_owned(),
        position: bouten_position_str(b.position).to_owned(),
        range: None,
    }
}

fn project_tcy(t: &TateChuYoko<'_>) -> IrInline {
    IrInline::Tcy {
        text: content_to_string(t.text.get()),
        range: None,
    }
}

fn project_gaiji(g: &AozoraGaiji<'_>) -> IrInline {
    IrInline::Gaiji {
        codepoint: g.ucs.map(resolved_to_string),
        description: (!g.description.is_empty()).then(|| g.description.to_owned()),
        fallback_text: None,
        range: None,
    }
}

fn project_annotation(a: &AozoraAnnotation<'_>) -> IrInline {
    IrInline::Annotation {
        payload: a.raw.as_str().to_owned(),
        resolved: annotation_kind_resolved(a.kind).map(str::to_owned),
        range: None,
    }
}

fn project_content_inlines(content: Content<'_>) -> Vec<IrInline> {
    match content {
        Content::Plain(s) if !s.is_empty() => vec![IrInline::Text {
            value: s.to_owned(),
            range: None,
        }],
        Content::Segments(segs) => {
            let mut out = Vec::with_capacity(segs.len());
            for seg in segs {
                match *seg {
                    Segment::Text(t) if !t.is_empty() => out.push(IrInline::Text {
                        value: t.to_owned(),
                        range: None,
                    }),
                    Segment::Gaiji(g) => out.push(project_gaiji(g)),
                    Segment::Annotation(a) => out.push(project_annotation(a)),
                    // Empty `Segment::Text` plus any future
                    // non-exhaustive variant: drop quietly.
                    _ => {}
                }
            }
            out
        }
        // `Content::Plain("")` plus any future non-exhaustive variant:
        // produce no IR.
        _ => Vec::new(),
    }
}

fn content_to_string(content: Content<'_>) -> String {
    match content {
        Content::Plain(s) => s.to_owned(),
        Content::Segments(segs) => {
            let mut out = String::new();
            for seg in segs {
                if let Segment::Text(t) = seg {
                    out.push_str(t);
                }
            }
            out
        }
        _ => String::new(),
    }
}

fn resolved_to_string(r: Resolved) -> String {
    match r {
        Resolved::Char(c) => c.to_string(),
        Resolved::Multi(s) => s.to_owned(),
    }
}

// All upstream payload enums are `#[non_exhaustive]`. The trailing
// wildcard arm fires only when a future upstream release adds a
// variant before afm bumps its dep, so we keep its return value
// **distinct** from every named variant: the wildcard returns
// `"unknown"` (or `None`), and named variants return their own
// semantic mapping. That way a future-variant hit is observable in
// the IR rather than silently coinciding with a known variant's
// output. Clippy's `match_same_arms` would otherwise flag any
// explicit arm that happens to share the wildcard body — but we
// don't have to silence the lint because our values are genuinely
// distinct everywhere.

const fn bouten_kind_str(k: BoutenKind) -> &'static str {
    match k {
        BoutenKind::Goma => "goma",
        BoutenKind::WhiteSesame => "whiteSesame",
        BoutenKind::Circle => "circle",
        BoutenKind::WhiteCircle => "whiteCircle",
        BoutenKind::DoubleCircle => "doubleCircle",
        BoutenKind::Janome => "janome",
        BoutenKind::Cross => "cross",
        BoutenKind::WhiteTriangle => "whiteTriangle",
        BoutenKind::WavyLine => "wavyLine",
        BoutenKind::UnderLine => "underLine",
        BoutenKind::DoubleUnderLine => "doubleUnderLine",
        _ => "unknown",
    }
}

const fn bouten_position_str(p: BoutenPosition) -> &'static str {
    match p {
        BoutenPosition::Right => "right",
        BoutenPosition::Left => "left",
        _ => "unknown",
    }
}

const fn section_kind_subtype(kind: SectionKind) -> &'static str {
    match kind {
        SectionKind::Choho => "choho",
        SectionKind::Dan => "dan",
        SectionKind::Spread => "spread",
        _ => "unknown",
    }
}

const fn container_subtype(kind: ContainerKind) -> &'static str {
    match kind {
        ContainerKind::Indent { .. } => "indent",
        ContainerKind::Warichu => "warichu",
        ContainerKind::Keigakomi => "keigakomi",
        ContainerKind::AlignEnd { .. } => "alignEnd",
        _ => "unknown",
    }
}

const fn container_indent_level(kind: ContainerKind) -> Option<u32> {
    // Only the size-carrying variants emit a depth. `Warichu` and
    // `Keigakomi` (and any future non-exhaustive variant) fall
    // through the wildcard with `None`.
    match kind {
        ContainerKind::Indent { amount } => Some(amount as u32),
        ContainerKind::AlignEnd { offset } => Some(offset as u32),
        _ => None,
    }
}

const fn annotation_kind_resolved(k: AnnotationKind) -> Option<&'static str> {
    // Named annotation kinds project to their camelCase tag.
    // `Unknown` deliberately differs from a future-variant hit:
    // `Some("unknown")` says the upstream classifier saw the
    // annotation but couldn't classify it, whereas `None` says afm
    // doesn't know about this variant of `AnnotationKind` yet.
    match k {
        AnnotationKind::Unknown => Some("unknown"),
        AnnotationKind::AsIs => Some("asIs"),
        AnnotationKind::TextualNote => Some("textualNote"),
        AnnotationKind::InvalidRubySpan => Some("invalidRubySpan"),
        AnnotationKind::WarichuOpen => Some("warichuOpen"),
        AnnotationKind::WarichuClose => Some("warichuClose"),
        _ => None,
    }
}

fn table_align(a: TableAlignment) -> IrTableAlign {
    match a {
        TableAlignment::Left => IrTableAlign::Left,
        TableAlignment::Center => IrTableAlign::Center,
        TableAlignment::Right => IrTableAlign::Right,
        TableAlignment::None => IrTableAlign::Default,
    }
}

fn sourcepos_to_range(s: &Sourcepos) -> Option<Range> {
    // comrak source positions are 1-based line/column. We convert to
    // a pseudo-byte range by collapsing line numbers — the HTML
    // output doesn't carry true byte offsets, so the range here is
    // best-effort.
    let from = to_u32(
        s.start
            .line
            .saturating_sub(1)
            .saturating_mul(1024)
            .saturating_add(s.start.column.saturating_sub(1)),
    );
    let to = to_u32(
        s.end
            .line
            .saturating_sub(1)
            .saturating_mul(1024)
            .saturating_add(s.end.column.saturating_sub(1)),
    );
    (to >= from).then_some(Range { from, to })
}

#[cfg(test)]
mod tests {
    //! Unit tests for the pure projection helpers.
    //!
    //! These cover the match arms inside the `const fn` projectors
    //! that are otherwise reachable only through specific Aozora
    //! input patterns — enumerating every input grammar in
    //! integration tests would be both noisy and fragile against
    //! upstream parser evolution. Calling the projectors directly
    //! with synthetic enum values pins each match arm to a value, so
    //! an upstream rename or variant removal fails the build at the
    //! call site rather than silently in the IR.

    use super::*;
    use aozora_syntax::AlignEnd;
    use comrak::nodes::{LineColumn, Sourcepos};

    #[test]
    fn bouten_kind_str_covers_every_upstream_variant() {
        let cases = [
            (BoutenKind::Goma, "goma"),
            (BoutenKind::WhiteSesame, "whiteSesame"),
            (BoutenKind::Circle, "circle"),
            (BoutenKind::WhiteCircle, "whiteCircle"),
            (BoutenKind::DoubleCircle, "doubleCircle"),
            (BoutenKind::Janome, "janome"),
            (BoutenKind::Cross, "cross"),
            (BoutenKind::WhiteTriangle, "whiteTriangle"),
            (BoutenKind::WavyLine, "wavyLine"),
            (BoutenKind::UnderLine, "underLine"),
            (BoutenKind::DoubleUnderLine, "doubleUnderLine"),
        ];
        for (kind, expected) in cases {
            assert_eq!(bouten_kind_str(kind), expected);
        }
    }

    #[test]
    fn bouten_position_str_covers_left_and_right() {
        assert_eq!(bouten_position_str(BoutenPosition::Right), "right");
        assert_eq!(bouten_position_str(BoutenPosition::Left), "left");
    }

    #[test]
    fn section_kind_subtype_covers_every_upstream_variant() {
        assert_eq!(section_kind_subtype(SectionKind::Choho), "choho");
        assert_eq!(section_kind_subtype(SectionKind::Dan), "dan");
        assert_eq!(section_kind_subtype(SectionKind::Spread), "spread");
    }

    #[test]
    fn container_subtype_and_indent_level_round_trip_each_variant() {
        let indent = ContainerKind::Indent { amount: 3 };
        assert_eq!(container_subtype(indent), "indent");
        assert_eq!(container_indent_level(indent), Some(3));

        let align = ContainerKind::AlignEnd {
            offset: AlignEnd { offset: 1 }.offset,
        };
        assert_eq!(container_subtype(align), "alignEnd");
        assert_eq!(container_indent_level(align), Some(1));

        assert_eq!(container_subtype(ContainerKind::Warichu), "warichu");
        assert!(container_indent_level(ContainerKind::Warichu).is_none());
        assert_eq!(container_subtype(ContainerKind::Keigakomi), "keigakomi");
        assert!(container_indent_level(ContainerKind::Keigakomi).is_none());
    }

    #[test]
    fn annotation_kind_resolved_covers_every_named_variant() {
        // `Unknown` is the upstream classifier's "tried, gave up"
        // outcome; we surface it as `Some("unknown")` so consumers
        // distinguish it from a future-variant hit (`None`).
        assert_eq!(
            annotation_kind_resolved(AnnotationKind::Unknown),
            Some("unknown")
        );
        assert_eq!(annotation_kind_resolved(AnnotationKind::AsIs), Some("asIs"));
        assert_eq!(
            annotation_kind_resolved(AnnotationKind::TextualNote),
            Some("textualNote")
        );
        assert_eq!(
            annotation_kind_resolved(AnnotationKind::InvalidRubySpan),
            Some("invalidRubySpan")
        );
        assert_eq!(
            annotation_kind_resolved(AnnotationKind::WarichuOpen),
            Some("warichuOpen")
        );
        assert_eq!(
            annotation_kind_resolved(AnnotationKind::WarichuClose),
            Some("warichuClose")
        );
    }

    #[test]
    fn resolved_to_string_handles_char_and_multi() {
        assert_eq!(resolved_to_string(Resolved::Char('a')), "a");
        assert_eq!(resolved_to_string(Resolved::Multi("か゚")), "か゚");
    }

    #[test]
    fn project_content_inlines_covers_plain_segments_and_empty() {
        assert!(project_content_inlines(Content::Plain("")).is_empty());
        let plain = project_content_inlines(Content::Plain("hi"));
        assert!(matches!(
            plain.as_slice(),
            [IrInline::Text { value, .. }] if value == "hi"
        ));

        let segs: &[Segment<'_>] = &[Segment::Text("a"), Segment::Text("")];
        let segs_out = project_content_inlines(Content::Segments(segs));
        // Empty Text drops; non-empty survives.
        assert_eq!(segs_out.len(), 1);
    }

    #[test]
    fn content_to_string_concatenates_segment_text_only() {
        assert_eq!(content_to_string(Content::Plain("xyz")), "xyz");
        let segs: &[Segment<'_>] = &[Segment::Text("a"), Segment::Text("b")];
        assert_eq!(content_to_string(Content::Segments(segs)), "ab");
    }

    #[test]
    fn table_align_maps_every_alignment() {
        assert!(matches!(
            table_align(TableAlignment::Left),
            IrTableAlign::Left
        ));
        assert!(matches!(
            table_align(TableAlignment::Center),
            IrTableAlign::Center
        ));
        assert!(matches!(
            table_align(TableAlignment::Right),
            IrTableAlign::Right
        ));
        assert!(matches!(
            table_align(TableAlignment::None),
            IrTableAlign::Default
        ));
    }

    #[test]
    fn sourcepos_to_range_returns_some_for_well_ordered_positions() {
        let pos = Sourcepos {
            start: LineColumn { line: 1, column: 1 },
            end: LineColumn { line: 1, column: 5 },
        };
        let range = sourcepos_to_range(&pos).expect("forward range");
        assert!(range.from <= range.to);
    }

    #[test]
    fn sourcepos_to_range_returns_none_for_inverted_positions() {
        // Constructed (impossible) inverted sourcepos: start later
        // than end. The helper guards against negative ranges by
        // returning `None`, which keeps the IR robust under malformed
        // upstream output.
        let pos = Sourcepos {
            start: LineColumn { line: 5, column: 5 },
            end: LineColumn { line: 1, column: 1 },
        };
        assert!(sourcepos_to_range(&pos).is_none());
    }
}
