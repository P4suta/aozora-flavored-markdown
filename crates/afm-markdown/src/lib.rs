//! Aozora Flavored Markdown — CommonMark + GFM + 青空文庫記法.
//!
//! Layers `aozora-pipeline` (青空文庫記法 borrowed-AST lexer) onto a
//! vendored verbatim comrak so a single [`render_to_string`] call
//! turns afm source into HTML. Public entry points:
//!
//! - [`render_to_string`] — render afm source straight to HTML.
//! - [`serialize`] — afm-source round-trip (delegates to
//!   [`aozora_render::serialize::serialize`]).
//! - [`Options`] — configuration; [`Options::afm_default`] enables
//!   the GFM extensions afm uses on top of CommonMark.
//!
//! ## Pipeline
//!
//! ```text
//! source                                   ── UTF-8 input
//!   │
//!   ▼ aozora_pipeline::lex_into_arena      ── normalized text + Registry
//!   │
//!   ▼ comrak::parse_document               ── vanilla CommonMark + GFM
//!   │   (PUA sentinels U+E001..U+E004 flow through as plain text)
//!   │
//!   ▼ comrak::format_html_with_options     ── HTML with sentinels
//!   │
//!   ▼ ast_splice::splice_into_ast          ── sentinel → aozora-render,
//!   │   · INLINE_SENTINEL → NodeValue::Raw inline node
//!   │   · BLOCK_LEAF paragraphs → NodeValue::Raw block node
//!   │   · BLOCK_OPEN/CLOSE paragraphs → container open/close raws
//!   │
//!   ▼ comrak::format_html                  ── vanilla, sentinel-free AST
//!   │
//!   ▼
//! HTML
//! ```
//!
//! Comrak is unmodified: the v0.52.0 verbatim tree carries no
//! Aozora-aware code (ADR-0001 budget = 0).

#![forbid(unsafe_code)]

mod ast_splice;
mod code_block_mask;
pub mod html;
pub mod ir;
mod sentinel_stream;
mod source_line_anchors;

/// PUA sentinel codepoints embedded by `aozora_pipeline`.
///
/// Re-exported here under afm-side names so afm's public API never
/// names sibling crate constants — if the upstream renames or removes
/// one of these, the change surfaces in this module instead of
/// breaking every downstream consumer.
pub mod sentinels {
    /// Inline Aozora span (ruby / bouten / annotation / gaiji /
    /// TCY / kaeriten).
    pub const INLINE: char = aozora_pipeline::INLINE_SENTINEL;
    /// Block-leaf Aozora line (page break, section break, leaf
    /// indent, sashie).
    pub const BLOCK_LEAF: char = aozora_pipeline::BLOCK_LEAF_SENTINEL;
    /// Paired-container open line (e.g. `［＃ここから字下げ］`).
    pub const BLOCK_OPEN: char = aozora_pipeline::BLOCK_OPEN_SENTINEL;
    /// Paired-container close line (e.g. `［＃ここで字下げ終わり］`).
    pub const BLOCK_CLOSE: char = aozora_pipeline::BLOCK_CLOSE_SENTINEL;
}

pub use aozora_spec::{Diagnostic, DiagnosticSource, Severity};

use core::mem;

use aozora_render::serialize as aozora_serialize;
use aozora_syntax::borrowed::Arena;
use comrak::nodes::AstNode;

/// Parse-time configuration for [`render_to_string`] and friends.
///
/// `comrak::Options` is held with a `'static` lifetime: afm doesn't
/// install URL rewriters or broken-link callbacks (which are the
/// only comrak fields that need a non-`'static` lifetime), so the
/// borrow parameter would be dead weight in our public API.
#[derive(Debug, Clone, Default)]
pub struct Options {
    pub comrak: comrak::Options<'static>,
    /// When `true`, run the aozora lex pre-pass and HTML
    /// post-processing. When `false`, the input flows straight into
    /// vanilla `comrak::parse_document` + `format_html` — used by the
    /// CommonMark / GFM spec conformance runners to verify the wrapper
    /// does not perturb upstream behaviour.
    pub aozora_enabled: bool,
    /// When `true`, the HTML renderer adds `data-afm-source-line="N"`
    /// (1-based) to every top-level block element it emits. The
    /// afm-obsidian document-mode adapter (Pillar 6 of the plan)
    /// uses these anchors to map per-block post-processor calls back
    /// to slices of the rendered fragment without re-parsing.
    ///
    /// Defaults to `false`. Cost when enabled: one extra walk over
    /// comrak's top-level AST children + a streaming insert pass on
    /// the produced HTML. Both are O(blocks).
    pub source_line_anchors: bool,
}

impl Options {
    /// Default afm configuration: GFM extensions on (strikethrough,
    /// table, autolink, tasklist), hardbreaks on so each Aozora source
    /// newline becomes a `<br>` (verse / dialogue boundaries are
    /// load-bearing in 青空文庫 source).
    #[must_use]
    pub fn afm_default() -> Self {
        let mut comrak = comrak::Options::default();
        comrak.extension.strikethrough = true;
        comrak.extension.table = true;
        comrak.extension.autolink = true;
        comrak.extension.tasklist = true;
        comrak.render.hardbreaks = true;
        Self {
            comrak,
            aozora_enabled: true,
            source_line_anchors: false,
        }
    }

    /// Plain CommonMark (no GFM, no Aozora, raw HTML enabled). Used by
    /// the CommonMark 0.31.2 spec-conformance test to verify the
    /// wrapper does not perturb comrak's CommonMark behaviour.
    #[must_use]
    pub fn commonmark_only() -> Self {
        let mut comrak = comrak::Options::default();
        comrak.render.r#unsafe = true;
        Self {
            comrak,
            aozora_enabled: false,
            source_line_anchors: false,
        }
    }

    /// Pure-GFM extension set (no Aozora, raw HTML enabled). Used by
    /// the GFM 0.29 spec-conformance test.
    #[must_use]
    pub fn gfm_only() -> Self {
        let mut comrak = comrak::Options::default();
        comrak.extension.strikethrough = true;
        comrak.extension.table = true;
        comrak.extension.autolink = true;
        comrak.extension.tasklist = true;
        comrak.extension.tagfilter = true;
        comrak.render.r#unsafe = true;
        Self {
            comrak,
            aozora_enabled: false,
            source_line_anchors: false,
        }
    }

    /// Builder-style toggle for source-line anchors. Returns a new
    /// `Options` with `source_line_anchors = on`.
    ///
    /// ```
    /// use afm_markdown::Options;
    /// let opts = Options::afm_default().with_source_line_anchors(true);
    /// assert!(opts.source_line_anchors);
    /// ```
    #[must_use]
    pub fn with_source_line_anchors(mut self, on: bool) -> Self {
        self.source_line_anchors = on;
        self
    }
}

/// Output of [`render_to_string`].
#[derive(Debug)]
pub struct Rendered {
    /// HTML output, with every Aozora sentinel substituted.
    pub html: String,
    /// Non-fatal lexer observations (unclosed pairs, PUA collisions,
    /// stray triggers, …). Empty on the happy path.
    pub diagnostics: Vec<Diagnostic>,
}

/// Output of [`render_to_ir`].
///
/// The IR projection alongside the HTML and diagnostics. Used by the
/// `afm-wasm` bridge so the JS-side renderer can pick its own output
/// target (DOM fragment, `CodeMirror` `RangeSet`, semantic tokens, …)
/// from a single source.
#[derive(Debug)]
pub struct RenderedIr {
    pub ir: ir::IrDocument,
    pub html: String,
    pub diagnostics: Vec<Diagnostic>,
}

/// Render afm source text to HTML.
///
/// One-stop entry point for the typical caller (afm CLI, afm-epub).
/// Internally:
///
/// 1. [`aozora_pipeline::lex_into_arena`] turns the source into a normalized
///    text (with PUA sentinels at every Aozora construct) plus a
///    borrowed `Registry`.
/// 2. `comrak::parse_document` + `comrak::format_html` runs against
///    the normalized text — sentinels flow through as plain text since
///    they are not in CommonMark's escape set (`<`/`>`/`&`/`"`).
/// 3. The internal `post_process` module sweeps the produced HTML,
///    substituting each sentinel with the matching
///    `aozora_render::render_node` output.
///
/// # Panics
///
/// Panics if `comrak::format_html` fails to write into the internal
/// `String` sink — `String` cannot fail as a `fmt::Write`, so this
/// branch is unreachable in normal use.
#[must_use]
pub fn render_to_string(input: &str, options: &Options) -> Rendered {
    let (html, diagnostics, ()) = drive_pipeline(input, options, |_root, _lex_out| ());
    Rendered { html, diagnostics }
}

/// Render afm source to a structured IR + HTML + diagnostics.
///
/// Mirrors [`render_to_string`] but additionally walks comrak's AST
/// to emit a typed [`ir::IrDocument`]. The IR is the canonical
/// contract between afm-wasm and afm-obsidian's TS renderers.
///
/// The IR covers the full Markdown side (paragraph, heading,
/// blockquote, list, code, thematic break, table, image) and the
/// full Aozora side (`Ruby` / `DoubleRuby` / `Bouten` / `Tcy` /
/// `Gaiji` / `Annotation` / `PageBreak` / `SectionBreak` /
/// `Container`); heading hints (`［＃「X」は大見出し］`) promote
/// their host paragraph to `IrBlock::Heading` so the IR shape
/// matches the rendered HTML one-for-one.
///
/// # Panics
///
/// Panics if `comrak::format_html` fails to write into the internal
/// `String` sink — `String` cannot fail as a `fmt::Write`, so this
/// branch is unreachable in normal use.
#[must_use]
pub fn render_to_ir(input: &str, options: &Options) -> RenderedIr {
    let (html, diagnostics, ir) = drive_pipeline(input, options, ir::build_ir);
    RenderedIr {
        ir,
        html,
        diagnostics,
    }
}

/// Internal pipeline driver shared between `render_to_string` and
/// `render_to_ir`.
///
/// Runs the full lex → comrak → format → post-process → unmask →
/// anchors chain and threads the AST root + optional `BorrowedLexOutput`
/// through `project` *before* HTML formatting starts. The closure
/// returns whatever extra data the caller needs alongside the HTML
/// (`()` for the plain renderer, an `IrDocument` for the IR
/// renderer).
fn drive_pipeline<F, T>(input: &str, options: &Options, project: F) -> (String, Vec<Diagnostic>, T)
where
    F: for<'a> FnOnce(&'a AstNode<'a>, Option<&aozora_pipeline::BorrowedLexOutput<'a>>) -> T,
{
    if !options.aozora_enabled {
        let comrak_arena = comrak::Arena::new();
        let root = comrak::parse_document(&comrak_arena, input, &options.comrak);
        let extra = project(root, None);
        let html = format_root(root, options, None);
        return (html, Vec::new(), extra);
    }

    // Pre-process: hide aozora trigger characters that live inside a
    // CommonMark fenced code block from the lexer. `aozora_pipeline` is
    // CommonMark-blind by design (ADR-0010), so this lives here. See
    // `code_block_mask` module docs for the masking scheme.
    let (masked_source, mask_originals) = code_block_mask::mask_code_block_triggers(input);

    let arena = Arena::new();
    let lex_out = aozora_pipeline::lex_into_arena(&masked_source, &arena);

    let comrak_arena = comrak::Arena::new();
    let root = comrak::parse_document(&comrak_arena, lex_out.normalized, &options.comrak);

    // IR projection sees the AST *before* sentinel splicing — it
    // walks the same Text-with-sentinel-char pre-mutation tree the
    // splicer is about to consume. Both walkers share
    // `SentinelCursor` primitives (each materialises its own cursor)
    // so they stay in lockstep without serial coupling.
    let extra = project(root, Some(&lex_out));

    // Mutate the AST: every PUA sentinel becomes a `NodeValue::Raw`
    // node carrying the rendered Aozora HTML. After this returns,
    // the AST contains no sentinel character; `comrak::format_html`
    // emits final HTML in a single verbatim pass.
    ast_splice::splice_into_ast(root, &comrak_arena, &lex_out);

    let html = format_root(root, options, Some(mask_originals.as_slice()));
    (html, lex_out.diagnostics, extra)
}

/// Common HTML finalisation: comrak-format the root (per top-level
/// child when `source_line_anchors` is on, so each child's first
/// open tag picks up its `data-afm-source-line` attribute), then
/// unmask code-block triggers.
///
/// AST-level Aozora sentinel splicing runs in [`drive_pipeline`]
/// before this is called, so by the time we hand the AST to
/// `comrak::format_html` no PUA sentinel remains.
fn format_root<'a>(
    root: &'a AstNode<'a>,
    options: &Options,
    mask_originals: Option<&[char]>,
) -> String {
    let html = if options.source_line_anchors {
        source_line_anchors::format_root_with_anchors(root, &options.comrak)
    } else {
        let mut html = String::new();
        comrak::format_html(root, &options.comrak, &mut html)
            .expect("formatting to a String never fails");
        html
    };
    if let Some(originals) = mask_originals {
        code_block_mask::unmask_html(&html, originals).into_owned()
    } else {
        html
    }
}

/// One block of [`render_blocks_to_ir`]'s output.
///
/// Each entry corresponds to one top-level comrak child. `html` is the
/// rendered HTML for that child (with Aozora sentinels spliced).
/// `ir` is the IR projection — typically a single block, but may be
/// empty for comrak constructs without a v0.2 IR mapping (definition
/// lists, footnote refs, raw HTML, etc.) and may carry more than one
/// block when an Aozora paired-container drains at the call boundary.
#[derive(Debug, Clone)]
pub struct RenderedBlock {
    pub ir: Vec<ir::IrBlock>,
    pub html: String,
    /// 1-based line where this block began in the source.
    pub source_line: u32,
}

/// Per-block streaming render.
///
/// Produces one [`RenderedBlock`] per top-level comrak child, in
/// document order. Used by afm-obsidian's chunked-cancellation path
/// (ADR-0009): the JS bridge can iterate the returned vector and
/// check its `AbortSignal` between blocks.
///
/// The current implementation parses the document once (a single
/// comrak pass) and renders each top-level block's HTML separately
/// using `comrak::format_html`. Diagnostics from the lexer are
/// returned alongside the blocks, attached to the document as a
/// whole rather than per-block (the lexer pass is non-block-scoped).
///
/// Limitation: container constructs that span multiple top-level
/// blocks (e.g., `［＃ここから２字下げ］`...`［＃ここで字下げ終わり］`)
/// are emitted as separate blocks; the consumer is responsible for
/// re-assembling them. The whole-document `render_to_ir` path
/// preserves cross-block structure if you need it.
#[must_use]
pub fn render_blocks_to_ir(
    input: &str,
    options: &Options,
) -> (Vec<RenderedBlock>, Vec<Diagnostic>) {
    if !options.aozora_enabled {
        let comrak_arena = comrak::Arena::new();
        let root = comrak::parse_document(&comrak_arena, input, &options.comrak);
        let blocks = collect_rendered_blocks(root, options, Vec::new());
        return (blocks, Vec::new());
    }

    let (masked_source, _mask_originals) = code_block_mask::mask_code_block_triggers(input);
    let arena = Arena::new();
    let lex_out = aozora_pipeline::lex_into_arena(&masked_source, &arena);
    let comrak_arena = comrak::Arena::new();
    let root = comrak::parse_document(&comrak_arena, lex_out.normalized, &options.comrak);
    // IR projection runs before AST mutation so it walks the
    // sentinel-bearing Text nodes; AST splicing afterwards rewrites
    // the same nodes for `comrak::format_html` consumption. A single
    // `StreamingIrBuilder` threads its cursor across every top-level
    // child so the registry stays in lockstep — a per-call builder
    // would restart the cursor at 0 for every block and misalign
    // Aozora projection against the registry.
    let blocks_ir: Vec<Vec<ir::IrBlock>> = {
        let mut builder = ir::StreamingIrBuilder::new(Some(&lex_out));
        root.children()
            .map(|child| builder.walk_block(child))
            .collect()
    };
    ast_splice::splice_into_ast(root, &comrak_arena, &lex_out);
    let blocks = collect_rendered_blocks(root, options, blocks_ir);
    (blocks, lex_out.diagnostics)
}

fn collect_rendered_blocks<'a>(
    root: &'a AstNode<'a>,
    options: &Options,
    mut blocks_ir: Vec<Vec<ir::IrBlock>>,
) -> Vec<RenderedBlock> {
    // The AST has already been spliced at the document level by the
    // caller (so `format_html` sees no sentinels here), and the IR
    // was already projected from the *pre-splice* AST in source
    // order. We zip them back together one block at a time.
    //
    // Pure-markdown mode (`Options::aozora_enabled = false`) hands
    // us an empty IR vector; we emit `Vec::new()` per block in that
    // case so the per-block IR field stays consistent with the IR
    // builder's no-op behaviour.
    let mut blocks = Vec::new();
    for (idx, child) in root.children().enumerate() {
        let data = child.data.borrow();
        let line = sentinel_stream::saturating_u32(data.sourcepos.start.line).max(1);
        drop(data);
        let mut block_html = String::new();
        comrak::format_html(child, &options.comrak, &mut block_html)
            .expect("formatting a String never fails");
        let ir_blocks = if idx < blocks_ir.len() {
            mem::take(&mut blocks_ir[idx])
        } else {
            Vec::new()
        };
        blocks.push(RenderedBlock {
            ir: ir_blocks,
            html: block_html,
            source_line: line,
        });
    }
    blocks
}

/// Round-trip an afm source through the lexer and back to canonical
/// afm-source text.
///
/// Delegates to [`aozora_render::serialize::serialize`] — the
/// borrowed-AST inverse of `lex_into_arena`. Plain CommonMark portions
/// of the input pass through verbatim because the lexer leaves them
/// untouched.
#[must_use]
pub fn serialize(input: &str) -> String {
    let arena = Arena::new();
    let lex_out = aozora_pipeline::lex_into_arena(input, &arena);
    aozora_serialize::serialize(&lex_out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_round_trips_through_html() {
        let r = render_to_string("hello, world", &Options::afm_default());
        assert!(r.html.contains("hello, world"), "html: {}", r.html);
        assert!(r.diagnostics.is_empty());
    }

    #[test]
    fn plain_text_serialize_returns_input_unchanged() {
        assert_eq!(serialize("plain text"), "plain text");
    }

    #[test]
    fn ruby_renders_as_html_ruby_element() {
        let r = render_to_string("｜青梅《おうめ》へ", &Options::afm_default());
        assert!(r.html.contains("<ruby>"), "html: {}", r.html);
        assert!(r.html.contains("青梅"));
        assert!(r.html.contains("おうめ"));
        // No bare ［＃ leak (Tier-A canary).
        assert!(!r.html.contains("［＃"));
    }

    #[test]
    fn page_break_promotes_and_does_not_leak_brackets() {
        let r = render_to_string("前［＃改ページ］後", &Options::afm_default());
        assert!(!r.html.contains("［＃"), "html: {}", r.html);
    }

    #[test]
    fn unknown_annotation_keeps_brackets_inside_wrapper() {
        let r = render_to_string("前［＃ほげふが］後", &Options::afm_default());
        // The annotation HTML carries the original text inside an
        // `afm-annotation` wrapper, so the bracket character may
        // appear, but never bare in body text.
        assert!(
            !contains_bare_bracket(&r.html),
            "bare bracket leaked in: {}",
            r.html
        );
    }

    #[test]
    fn commonmark_passes_through_with_heading_intact() {
        let r = render_to_string("# Hello\n\nworld", &Options::afm_default());
        assert!(r.html.contains("<h1>Hello</h1>"), "html: {}", r.html);
        assert!(r.html.contains("world"));
    }

    #[test]
    fn gfm_only_options_have_aozora_disabled_and_gfm_extensions_enabled() {
        let opts = Options::gfm_only();
        assert!(!opts.aozora_enabled, "gfm_only must skip the aozora pass");
        assert!(opts.comrak.extension.strikethrough);
        assert!(opts.comrak.extension.table);
        assert!(opts.comrak.extension.autolink);
        assert!(opts.comrak.extension.tasklist);
        assert!(opts.comrak.extension.tagfilter);
        assert!(opts.comrak.render.r#unsafe);
    }

    #[test]
    fn gfm_only_renders_strikethrough_and_does_not_recognise_ruby() {
        // gfm_only's contract: GFM extensions on, Aozora pre-pass off.
        // The strikethrough must produce `<del>`; the ruby-shaped
        // `｜...《》` source must survive verbatim because the lexer
        // never ran.
        let opts = Options::gfm_only();
        let html = render_to_string("~~strike~~ ｜青梅《おうめ》", &opts).html;
        assert!(html.contains("<del>strike</del>"), "html: {html}");
        assert!(
            html.contains("｜青梅"),
            "ruby trigger must survive raw: {html}"
        );
        assert!(
            !html.contains("<ruby>"),
            "ruby must NOT render in gfm-only: {html}"
        );
    }

    #[test]
    fn contains_bare_bracket_helper_detects_leaked_marker() {
        // Pins the "bare bracket leaked" branch of the helper itself.
        // The needle appears outside any tag and outside an
        // `afm-annotation` wrapper.
        assert!(contains_bare_bracket("plain ［＃ leak"));
        assert!(!contains_bare_bracket(
            "<span class=\"afm-annotation\" hidden>［＃</span>"
        ));
        assert!(!contains_bare_bracket("no marker at all"));
    }

    /// Tier-A canary: every occurrence of `［＃` must be inside an
    /// `afm-annotation` wrapper — never in raw body text.
    fn contains_bare_bracket(html: &str) -> bool {
        let needle = "［＃";
        let wrapper_open = "afm-annotation";
        let mut pos = 0;
        while let Some(idx) = html[pos..].find(needle) {
            let abs = pos + idx;
            let prefix = &html[..abs];
            let last_open = prefix.rfind('<').unwrap_or(0);
            let last_close = prefix.rfind('>').unwrap_or(0);
            let inside_tag = last_open > last_close;
            let in_wrapper = prefix.contains(wrapper_open);
            if !inside_tag && !in_wrapper {
                return true;
            }
            pos = abs + needle.len();
        }
        false
    }
}
