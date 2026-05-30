//! Public IR type definitions.
//!
//! Every type here is part of the `afm_markdown::ir` public surface
//! and feeds the TypeScript `IRDocument` consumed by
//! `afm-obsidian/src/ir/types.ts`. Keeping the names and field
//! ordering aligned across the FFI boundary makes the
//! `serde-wasm-bindgen` round-trip a pass-through, no shape
//! adapters needed.

use serde::Serialize;

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IrDocument {
    pub blocks: Vec<IrBlock>,
    pub diagnostics: Vec<IrDiagnostic>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
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
    /// `aozora::syntax::SectionKind`). `［＃改ページ］` is its own block — see
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
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
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
    /// `［＃` and `］`. `resolved` carries the
    /// `aozora::syntax::AnnotationKind` camelCase tag (`"unknown"`,
    /// `"asIs"`, `"textualNote"`, `"invalidRubySpan"`, `"warichuOpen"`,
    /// `"warichuClose"`) when the upstream lexer classified the
    /// annotation; `None` for future non-exhaustive variants afm
    /// hasn't seen yet.
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

/// Source-position range, end-exclusive.
///
/// `start` and `end` carry 1-based line / column coordinates straight
/// from comrak's `Sourcepos`. JS-side consumers (afm-obsidian's
/// `CodeMirror` bridge) can map these to editor positions without
/// re-doing UTF-8 byte arithmetic, which the previous pseudo-byte
/// representation silently broke for multi-byte CJK content.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

/// 1-based line / column tuple. `column` is a UTF-8 grapheme-blind
/// column count (matching comrak's `Sourcepos`), so it is suitable
/// for editor surfaces but not for byte slicing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    pub line: u32,
    pub column: u32,
}
