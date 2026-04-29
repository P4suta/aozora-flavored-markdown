//! Aozora Flavored Markdown — CommonMark + GFM + 青空文庫記法.
//!
//! Layers `aozora-lex` (青空文庫記法 borrowed-AST lexer) onto a
//! vendored verbatim comrak so a single [`render_to_string`] call
//! turns afm source into HTML. Public entry points:
//!
//! - [`render_to_string`] — render afm source straight to HTML.
//! - [`serialize`] — afm-source round-trip (delegates to
//!   [`aozora_render::serialize::serialize`]).
//! - [`Options`] — configuration; [`Options::afm_default`] enables
//!   the GFM extensions afm uses on top of CommonMark.
//!
//! ## Pipeline (post-v0.2.4 borrowed-AST migration)
//!
//! ```text
//! source                                   ── UTF-8 input
//!   │
//!   ▼ aozora_lex::lex_into_arena           ── normalized text + Registry
//!   │
//!   ▼ comrak::parse_document               ── vanilla CommonMark + GFM
//!   │   (PUA sentinels U+E001..U+E004 flow through as plain text)
//!   │
//!   ▼ comrak::format_html_with_options     ── HTML with sentinels
//!   │
//!   ▼ post_process::splice_aozora_html     ── sentinel → aozora-render
//!   │   · INLINE_SENTINEL → render_node::render output
//!   │   · BLOCK_LEAF paragraphs → leaf node HTML
//!   │   · BLOCK_OPEN/CLOSE paragraphs → container open/close
//!   │
//!   ▼
//! HTML
//! ```
//!
//! Comrak is unmodified: the v0.52.0 verbatim tree carries no
//! Aozora-aware code (ADR-0001 budget = 0).

#![forbid(unsafe_code)]

mod code_block_mask;
pub mod html;
mod post_process;

#[doc(hidden)]
pub mod test_support;

pub use aozora_lex::{
    BLOCK_CLOSE_SENTINEL, BLOCK_LEAF_SENTINEL, BLOCK_OPEN_SENTINEL, INLINE_SENTINEL,
};
pub use aozora_spec::Diagnostic;
pub use comrak::Options as ComrakOptions;

use aozora_render::serialize as aozora_serialize;
use aozora_syntax::borrowed::Arena;

/// Parse-time configuration for [`render_to_string`] and friends.
#[derive(Debug, Clone, Default)]
pub struct Options<'c> {
    pub comrak: comrak::Options<'c>,
    /// When `true`, run the aozora lex pre-pass and HTML
    /// post-processing. When `false`, the input flows straight into
    /// vanilla `comrak::parse_document` + `format_html` — used by the
    /// CommonMark / GFM spec conformance runners to verify the wrapper
    /// does not perturb upstream behaviour.
    pub aozora_enabled: bool,
}

impl Options<'_> {
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
        }
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

/// Render afm source text to HTML.
///
/// One-stop entry point for the typical caller (afm CLI, afm-epub).
/// Internally:
///
/// 1. [`aozora_lex::lex_into_arena`] turns the source into a normalized
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
pub fn render_to_string(input: &str, options: &Options<'_>) -> Rendered {
    if !options.aozora_enabled {
        let comrak_arena = comrak::Arena::new();
        let root = comrak::parse_document(&comrak_arena, input, &options.comrak);
        let mut html = String::new();
        comrak::format_html(root, &options.comrak, &mut html)
            .expect("formatting to a String never fails");
        return Rendered {
            html,
            diagnostics: Vec::new(),
        };
    }

    // Pre-process: hide aozora trigger characters that live inside a
    // CommonMark fenced code block from the lexer. `aozora_lex` is
    // CommonMark-blind by design (ADR-0010), so this lives here. See
    // `code_block_mask` module docs for the masking scheme.
    let (masked_source, mask_originals) = code_block_mask::mask_code_block_triggers(input);

    let arena = Arena::new();
    let lex_out = aozora_lex::lex_into_arena(&masked_source, &arena);

    let comrak_arena = comrak::Arena::new();
    let root = comrak::parse_document(&comrak_arena, lex_out.normalized, &options.comrak);
    let mut comrak_html = String::new();
    comrak::format_html(root, &options.comrak, &mut comrak_html)
        .expect("formatting to a String never fails");

    let spliced = post_process::splice_aozora_html(&comrak_html, &lex_out);
    let html = code_block_mask::unmask_html(&spliced, &mask_originals);

    Rendered {
        html,
        diagnostics: lex_out.diagnostics,
    }
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
    let lex_out = aozora_lex::lex_into_arena(input, &arena);
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
