# 0011. Brand boundary: HTML class rewrite at the afm side

- Status: accepted
- Date: 2026-05-04
- Tags: rendering, css, brand, sibling-repo

## Context

afm renders HTML by composing two black boxes:

1. **comrak** — vanilla CommonMark + GFM. Emits ordinary HTML (`<p>`,
   `<h1>`, `<ul>`, …) with no afm-specific styling.
2. **`aozora-render`** — the borrowed-AST renderer in the sibling
   `P4suta/aozora` repo. Emits HTML for ruby, bouten, gaiji, paired
   containers, and the rest of the 青空文庫 surface.

aozora-render was designed before afm existed. Its CSS class names
follow the **`aozora-*`** prefix (`aozora-ruby`, `aozora-bouten-goma`,
`aozora-page-break`, …) — its own brand for stand-alone 青空文庫 HTML
rendering. The sibling repo's mdbook theme, the
`aozora-tools` formatter, and any future direct consumer of
`aozora-render` all expect that prefix.

afm output is a *different* surface — Aozora Flavored Markdown — and
its public CSS contract uses the **`afm-*`** prefix (`afm-ruby`,
`afm-bouten-goma`, …). The mdbook theme under `crates/afm-book/theme/`
styles `afm-*` classes; downstream Obsidian / EPUB / browser renderers
target the same.

The two prefixes therefore have to be reconciled at the boundary
between aozora-render's output and afm's public HTML.

## Decision

Reconcile **on the afm side**, by post-processing aozora-render's HTML
fragments.

`crates/afm-markdown/src/post_process.rs::rebrand_aozora_classes_to_afm`
runs after sentinel splicing and rewrites every `aozora-*` token
inside a `class="…"` attribute to `afm-*`. It is a single linear pass
that touches only class attribute values; data attributes, link
targets, and text bodies survive verbatim.

## Why not parameterise `aozora-render` upstream?

The instinct is to push a `class_prefix: &str` configuration into
`aozora-render::render_node::render` so afm calls it with `"afm-"`
and never has to rewrite. We rejected this:

- **Coupling direction.** afm depends on aozora; the reverse must
  not hold. Adding a knob to aozora-render purely so afm can avoid a
  post-pass would make aozora's API surface accommodate a
  downstream's branding decision. That inverts the dependency model
  and constrains aozora's evolution.
- **aozora has its own independent reason to keep `aozora-*`.** The
  sibling repo's mdbook, the formatter, and any standalone consumer
  expect that brand. A configurable prefix would be a feature for
  one consumer (afm) at the cost of API surface for everyone.
- **The rewrite is cheap and correct.** A linear scan of the spliced
  HTML, idempotent, well-tested
  (`crates/afm-markdown/src/post_process.rs::tests`,
  property tests in
  `crates/afm-markdown/tests/property_html_shape.rs`,
  invariants in
  `crates/afm-markdown/src/test_support.rs::AFM_CLASSES`).
  The brand boundary is a real concept with real semantics; making
  it explicit in a single place is preferable to scattering it
  through aozora-render's internals.

## Consequences

- `aozora-render`'s output stays branded as `aozora-*` regardless of
  consumer. afm-tools and the sibling mdbook can keep depending on
  the `aozora-*` contract without coordination with afm.
- afm's public HTML carries only `afm-*` classes — verified by the
  `AFM_CLASSES` invariant and the property-test sweep.
- The rewrite pass adds one O(N) HTML scan per render. Measured cost
  is negligible against the surrounding parse + format work.
- Future aozora-render additions that introduce new `aozora-*`
  classes are picked up automatically: no afm changes required as
  long as the prefix is consistent.
- The brand boundary is **the only** afm-side rewrite of upstream
  HTML output (besides the Tier-A defensive `［＃` wrapper for
  unclaimed annotations). Adding more such rewrites would weaken the
  3-layer separation; if a future need arises, it should be
  discussed in a follow-up ADR rather than accreted silently.
