//! HTML rendering convenience entry.
//!
//! Back-compat shim for the pre-0.2.4 `html::render_to_string(src)`
//! signature. Equivalent to
//! `crate::render_to_string(src, &Options::afm_default()).html`.

use crate::{Options, render_to_string as render_into_rendered};

/// Render afm source to HTML, dropping diagnostics.
///
/// Convenience for the typical caller. For diagnostic-aware paths
/// (CLI `--strict` flag, LSP, corpus sweep) call
/// [`crate::render_to_string`] directly and inspect
/// [`crate::Rendered::diagnostics`].
#[must_use]
pub fn render_to_string(input: &str) -> String {
    render_into_rendered(input, &Options::afm_default()).html
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
