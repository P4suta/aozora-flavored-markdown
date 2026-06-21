//! `aozora-flavored-markdown-epub` — convert Aozora Flavored Markdown sources into an
//! EPUB 3.3 package. Spec-anchored: each phase cites the [EPUB 3.3]
//! section it implements.
//!
//! Pipeline ([`build`] is the only entry point):
//!
//! 1. **Discover** (`discover`) — collect Aozora Flavored Markdown
//!    sources + `book.toml`.
//! 2. **Render** (`render`) — Aozora Flavored Markdown source → XHTML
//!    spine item.
//! 3. **Compose** (`compose`) — OPF, Navigation Document, OCF
//!    `container.xml`, and the `mimetype` byte sequence.
//! 4. **Package** (`package`) — pack into the `.epub` ZIP container.
//!
//! [EPUB 3.3]: https://www.w3.org/TR/epub-33/

#![doc(html_logo_url = "https://github.com/P4suta/aozora-flavored-markdown-epub")]
#![forbid(unsafe_code)]

use std::path::Path;

mod compose;
mod discover;
mod error;
mod package;
mod render;

pub use error::{Error, Result};

/// Build configuration for one EPUB output.
#[derive(Debug, Clone)]
pub struct BuildOptions<'a> {
    /// Directory or single file containing Aozora Flavored Markdown sources.
    pub input: &'a Path,
    /// Path to `book.toml` metadata.
    pub metadata: &'a Path,
    /// Output `.epub` path.
    pub output: &'a Path,
}

/// Convert an Aozora Flavored Markdown manuscript into an EPUB 3.3 file.
///
/// # Errors
///
/// Returns [`Error`] if any phase fails. All errors carry source
/// spans where applicable (`miette::Report`).
pub fn build(opts: &BuildOptions<'_>) -> Result<()> {
    let manuscript = discover::collect(opts)?;
    let rendered = render::render_all(&manuscript)?;
    let bundle = compose::compose(&manuscript, &rendered)?;
    package::write(opts.output, &bundle)
}
