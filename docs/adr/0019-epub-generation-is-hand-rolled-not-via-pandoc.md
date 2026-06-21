# 0019. EPUB generation is hand-rolled (XHTML/CSS-native), not via a Pandoc bridge

- Status: accepted
- Date: 2026-06-22
- Deciders: @P4suta
- Tags: architecture, epub, output-formats

## Context

The sibling `aozora` parser produces non-HTML output by projecting its AST to a
**Pandoc AST** (`aozora-pandoc`) and shelling out to `pandoc` (`aozora pandoc -t
epub3`), which then writes EPUB/LaTeX/DOCX/etc. With the EPUB generator now in
this workspace ([ADR-0018](0018-consolidate-the-epub-generator-into-this-workspace.md)),
the question is whether to keep its hand-rolled containers or adopt the same
Pandoc bridge for ecosystem consistency and near-free extra formats.

The two lines have different centres of gravity:

- `aozora` (notation) projects to a generic AST and targets *breadth* of output
  formats, where per-format typographic fidelity matters less.
- This line (markdown → ebook) already emits XHTML directly and exists for
  *fidelity*: vertical writing (`writing-mode: vertical-rl`), ruby, bouten
  (`text-emphasis`), and tate-chu-yoko. Pandoc's generic EPUB writer serves
  Japanese vertical typesetting poorly, and a `pandoc` runtime dependency breaks
  the self-contained single-Rust-binary story.

## Decision

Keep the hand-rolled generator: `compose` builds the OPF / Navigation Document /
OCF `container.xml` through `quick_xml`, and `package` writes the `.epub` ZIP
(`mimetype` stored first, the rest deflated). No Pandoc projection, no `pandoc`
runtime dependency.

`aozora-flavored-markdown`'s XHTML fragment + the `aozora-md-*` CSS themes are
the universal intermediate for this line. Future paged output (PDF) is expected
to flow through the **same XHTML + CSS** via a CSS-paged-media engine
(Vivliostyle / Typst) — again Pandoc-free, reusing the canonical themes — added
as a sibling crate alongside `aozora-flavored-markdown-epub`.

## Consequences

- Precise EPUB 3.3 control: deterministic `page-progression-direction`,
  exact OCF byte layout (epubcheck-clean), and the vendored vertical/horizontal
  themes the renderer's classes require.
- No external `pandoc` binary at runtime; the generator stays a self-contained
  Rust crate/binary.
- DOCX / LaTeX / other "breadth" formats are **not** free here; each new output
  format is a deliberate sibling crate. That is the accepted price for fidelity.
- A deliberate, documented divergence from `aozora`'s Pandoc approach — the two
  product lines optimise for different things.

## Alternatives considered

- **Project to Pandoc AST and shell out to `pandoc` (mirror `aozora`).**
  Rejected: weaker Japanese vertical-typesetting fidelity, a heavy `pandoc`
  runtime dependency, and a lossy round-trip given we already emit XHTML.
- **Hybrid: hand-rolled EPUB, Pandoc only for breadth formats (DOCX/LaTeX).**
  Not adopted now (maintains two output stacks), but not precluded — a Pandoc
  bridge could be added later as its own crate if breadth is ever wanted.

## References

- [ADR-0018](0018-consolidate-the-epub-generator-into-this-workspace.md) — the consolidation this refines.
- aozora `aozora-pandoc` / `aozora pandoc` — the bridge approach this diverges from.
- EPUB 3.3 OCF / OPF / Navigation Document specifications cited inline in `compose.rs` / `package.rs`.
