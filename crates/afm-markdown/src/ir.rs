//! Intermediate representation produced by [`crate::render_to_ir`].
//!
//! The shape mirrors the TypeScript `IRDocument` consumed by
//! `afm-obsidian/src/ir/types.ts` (and validated in
//! `afm-obsidian/src/ir/from-wasm.ts`). Keeping the names and field
//! ordering aligned across the FFI boundary makes the
//! `serde-wasm-bindgen` round-trip a pass-through, no shape
//! adapters needed.
//!
//! # v0.1 scope
//!
//! This walker covers the **markdown-side** structure: paragraphs,
//! headings, lists, blockquotes, fenced code, tables, thematic
//! breaks. Inline runs preserve `Strong`, `Emphasis`, `Link`,
//! `Code`, `LineBreak`, and verbatim `Text` (the aozora-lexer's PUA
//! sentinels flow through as plain text in the IR for now).
//!
//! Aozora-specific IR nodes (Ruby / Bouten / Gaiji / TCY /
//! Annotation / Container / `PageBreak` / `SectionBreak`) are
//! consciously deferred to v0.2 once `aozora-render` exposes a
//! public arena walker; for now those constructs live in the
//! sibling HTML output.

use comrak::nodes::{
    AstNode, ListType, NodeHeading, NodeList, NodeValue, Sourcepos, TableAlignment,
};
use serde::Serialize;

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
    Container {
        /// `"indent" | "alignEnd" | "keigakomi" | "warichu"`
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
    SectionBreak {
        /// `"改丁" | "改見開き" | "改ページ"`
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
    LineBreak {
        hard: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        range: Option<Range>,
    },
    // ----- Aozora-specific variants (mirror TS IRInline) -----
    Ruby {
        base: Vec<Self>,
        reading: String,
        explicit: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        range: Option<Range>,
    },
    DoubleRuby {
        base: Vec<Self>,
        outer: String,
        inner: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        range: Option<Range>,
    },
    Bouten {
        children: Vec<Self>,
        /// `"sesame" | "circle" | "filledCircle" | "dot" | "triangle"`.
        style: String,
        /// `"over" | "under" | "right" | "left"`.
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

/// Walk a comrak AST root and project it to [`IrDocument`].
pub(crate) fn build_ir<'a>(root: &'a AstNode<'a>) -> IrDocument {
    let mut blocks = Vec::new();
    for child in root.children() {
        if let Some(block) = walk_block(child, true) {
            blocks.push(block);
        }
    }
    IrDocument {
        blocks,
        diagnostics: Vec::new(),
    }
}

/// Walk a single comrak block node (not the root) and project it to IR.
///
/// Used by `render_blocks_to_ir` for per-block streaming. Returns a
/// Vec because list / blockquote / etc. nest blocks; this flattens
/// to a single-element vec for top-level kinds.
pub fn walk_block_public<'a>(node: &'a AstNode<'a>) -> Vec<IrBlock> {
    walk_block(node, true).map_or_else(Vec::new, |b| vec![b])
}

fn walk_block<'a>(node: &'a AstNode<'a>, top_level: bool) -> Option<IrBlock> {
    let data = node.data.borrow();
    let source_line = top_level.then(|| to_u32(data.sourcepos.start.line).max(1));
    let range = sourcepos_to_range(&data.sourcepos);
    match &data.value {
        NodeValue::Paragraph => Some(IrBlock::Paragraph {
            children: collect_inlines(node),
            source_line,
            range,
        }),
        NodeValue::Heading(NodeHeading { level, .. }) => Some(IrBlock::Heading {
            level: (*level).clamp(1, 6),
            children: collect_inlines(node),
            source_line,
            range,
        }),
        NodeValue::BlockQuote => Some(IrBlock::Blockquote {
            children: collect_blocks(node),
            source_line,
            range,
        }),
        NodeValue::List(NodeList {
            list_type, start, ..
        }) => Some(IrBlock::List {
            ordered: matches!(list_type, ListType::Ordered),
            start: (*start > 1).then(|| to_u32(*start)),
            items: collect_list_items(node),
            source_line,
            range,
        }),
        NodeValue::CodeBlock(code) => Some(IrBlock::CodeBlock {
            lang: if code.info.is_empty() {
                None
            } else {
                Some(code.info.clone())
            },
            value: code.literal.clone(),
            source_line,
            range,
        }),
        NodeValue::ThematicBreak => Some(IrBlock::ThematicBreak { source_line, range }),
        NodeValue::Table(table) => {
            let mut rows: Vec<IrTableRow> = Vec::new();
            for child in node.children() {
                let row = collect_table_row(child);
                rows.push(row);
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
            let aligns: Vec<IrTableAlign> =
                table.alignments.iter().copied().map(table_align).collect();
            Some(IrBlock::Table {
                header,
                rows: body,
                align: aligns,
                source_line,
                range,
            })
        }
        // List items, table rows, and table cells are handled by their
        // parents above. Other unhandled block kinds (definition list,
        // footnote refs, etc.) are dropped from the IR for v0.1 — the
        // HTML output still has them.
        _ => None,
    }
}

fn collect_blocks<'a>(node: &'a AstNode<'a>) -> Vec<IrBlock> {
    let mut out = Vec::new();
    for child in node.children() {
        if let Some(block) = walk_block(child, false) {
            out.push(block);
        }
    }
    out
}

fn collect_list_items<'a>(node: &'a AstNode<'a>) -> Vec<IrListItem> {
    let mut out = Vec::new();
    for child in node.children() {
        let data = child.data.borrow();
        if matches!(data.value, NodeValue::Item(_)) {
            let range = sourcepos_to_range(&data.sourcepos);
            drop(data);
            out.push(IrListItem {
                children: collect_blocks(child),
                range,
            });
        }
    }
    out
}

fn collect_table_row<'a>(row: &'a AstNode<'a>) -> IrTableRow {
    let data = row.data.borrow();
    let range = sourcepos_to_range(&data.sourcepos);
    drop(data);
    let mut cells = Vec::new();
    for cell in row.children() {
        cells.push(collect_inlines(cell));
    }
    IrTableRow { cells, range }
}

fn table_align(a: TableAlignment) -> IrTableAlign {
    match a {
        TableAlignment::Left => IrTableAlign::Left,
        TableAlignment::Center => IrTableAlign::Center,
        TableAlignment::Right => IrTableAlign::Right,
        TableAlignment::None => IrTableAlign::Default,
    }
}

fn collect_inlines<'a>(node: &'a AstNode<'a>) -> Vec<IrInline> {
    let mut out = Vec::new();
    for child in node.children() {
        out.extend(walk_inline(child));
    }
    out
}

fn walk_inline<'a>(node: &'a AstNode<'a>) -> Option<IrInline> {
    let data = node.data.borrow();
    let range = sourcepos_to_range(&data.sourcepos);
    match &data.value {
        NodeValue::Text(s) => Some(IrInline::Text {
            value: s.to_string(),
            range,
        }),
        NodeValue::Code(c) => Some(IrInline::Code {
            value: c.literal.clone(),
            range,
        }),
        NodeValue::Strong => Some(IrInline::Strong {
            children: collect_inlines(node),
            range,
        }),
        NodeValue::Emph => Some(IrInline::Emphasis {
            children: collect_inlines(node),
            range,
        }),
        NodeValue::Link(link) => Some(IrInline::Link {
            href: link.url.clone(),
            title: (!link.title.is_empty()).then(|| link.title.clone()),
            children: collect_inlines(node),
            range,
        }),
        NodeValue::SoftBreak => Some(IrInline::LineBreak { hard: false, range }),
        NodeValue::LineBreak => Some(IrInline::LineBreak { hard: true, range }),
        // Image, footnote refs, raw HTML, etc. drop to text (preserves
        // visible content even if structural meaning is lost). v0.2
        // deepens this as needed.
        _ => None,
    }
}

fn sourcepos_to_range(s: &Sourcepos) -> Option<Range> {
    // comrak source positions are 1-based line/column. We convert to a
    // pseudo-byte range by collapsing line numbers — the HTML output
    // doesn't carry true byte offsets, so the range here is best-effort.
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
