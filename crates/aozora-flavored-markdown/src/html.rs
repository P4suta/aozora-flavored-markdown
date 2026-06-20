//! HTML rendering convenience entry.
//!
//! Convenience shim that renders straight to an HTML `String`. Equivalent
//! to `crate::render(src, &Options::default()).html`.

use crate::{Options, render as render_into_rendered};

/// Render aozora-flavored-markdown source to HTML, dropping diagnostics.
///
/// Convenience for the typical caller. For diagnostic-aware paths
/// (CLI `--strict` flag, LSP, corpus sweep) call
/// [`crate::render`] directly and inspect
/// [`crate::Rendered::diagnostics`].
///
/// # Examples
///
/// ```
/// let html = aozora_flavored_markdown::html::render_to_string("｜青梅《おうめ》");
/// assert!(html.contains("<ruby>"));
/// ```
#[must_use]
pub fn render_to_string(input: &str) -> String {
    render_into_rendered(input, &Options::default()).html
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_paragraph_round_trips_through_comrak() {
        let html = render_to_string("Hello.");
        assert!(html.contains("<p>Hello.</p>"));
    }

    #[test]
    fn ruby_is_emitted_semantically() {
        let html = render_to_string("｜青梅《おうめ》");
        assert!(html.contains("<ruby>"), "missing ruby tag: {html}");
        assert!(html.contains("青梅"));
        assert!(html.contains("おうめ"));
    }
}
