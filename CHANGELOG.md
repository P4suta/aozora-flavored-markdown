# Changelog

All notable changes to Aozora Flavored Markdown (afm) are recorded in
this file. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); the project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Re-enabled after v0.2.4 borrowed-AST migration

- 6 of the 11 integration tests are back in CI: `commonmark_spec`,
  `gfm_spec`, `css_class_contract`, `html_well_formed`,
  `block_structure_interaction` (1 case `#[ignore]`d as a v0.2.5 task —
  fenced code block contents need a CommonMark-aware lex skip), and
  `paired_container`. All 4 examples (`render-utf8` / `render-sjis` /
  `serialize-round-trip` / `ast-walk`) build against the new public API.
- `Options::aozora_enabled` flag added. `commonmark_only()` /
  `gfm_only()` set it to `false` so the spec runners exercise vanilla
  comrak without the lex pre-pass perturbing setext-heading and
  similar text-level constructs.
- `AFM_CLASSES` corrected to match what `aozora-render` v0.2.5 actually
  emits.

### Still gated for v0.2.5 (`#![cfg(any())]`)

- `heading_promotion` — needs paragraph-tag rewriting on top of the
  HTML post-process (HeadingHint inline → wrap host paragraph as
  `<h1>`/`<h2>`/`<h3>`).
- `post_process_invariants` — the proptest is shaped around the
  removed AST-surgery API and needs to be redrafted against HTML.
- `property_html_shape` — pathological inputs hit unbalanced
  container-close paragraphs (open-less `</div>`); needs a stack-aware
  splice in `post_process.rs`.
- `property_heading_integrity` — same dependency as `heading_promotion`.
- `aozora_parity` — the differential test against `aozora-render`'s own
  HTML output needs a fresh comparison harness.

## [0.2.4] - 2026-04-30

This release follows aozora `0.2.5` and completes the borrowed-AST
migration that began with the v0.2.0 split. afm-markdown is now a thin
glue crate that composes a vanilla comrak with `aozora-render` /
`aozora-lex` on a string-level sentinel substitution; comrak no longer
carries any Aozora-aware patches.

### Changed

- **comrak vendored tree is now 100 % verbatim v0.52.0.** The historical
  ~22-line patch surface (`NodeValue::Aozora` variant + `render_aozora`
  `fn` pointer + arms in cm/xml/html/sourcepos) has been removed, and
  the ADR-0001 200-line diff budget is now **0 lines**. Upstream syncs
  no longer need patch reapplication.
- **afm-markdown switched from owned-AST AST surgery to HTML
  post-processing.** The pipeline is now `aozora_lex::lex_into_arena` →
  `comrak::parse_document` (against the normalized text) →
  `comrak::format_html` → in-process sentinel substitution that calls
  `aozora_render::render_node` for every PUA-sentinel hit. See the
  module-level docs in `crates/afm-markdown/src/post_process.rs`.
- **Public API simplification.** The arena-coupled
  `parse(arena, input, options) -> ParseResult` and
  `serialize_from_artifacts(...)` entry points are replaced by
  `render_to_string(input, options) -> Rendered { html, diagnostics }`
  and `serialize(input) -> String`, both stateless and arena-free.
  `html::render_to_string` (no-arg shim returning `String`) is kept for
  back-compat.

### Removed

- `aozora-parser` dependency (the crate was retired in aozora 0.2.0
  Phase F.1).
- `aozora-lexer` direct dependency (afm-markdown only consumes
  `aozora-lex` now; the underlying `aozora-lexer` is pulled in
  transitively).
- `comrak::Options::extension::render_aozora` and `serialize_aozora`
  `fn` pointers.

### Internal

- 17 integration tests (`tests/*.rs`) and 4 examples were placed behind
  `#![cfg(any())]` for this release; the borrowed-AST rewrite of those
  fixtures is tracked under task #10 of the v0.2.4 release plan and
  will land in v0.2.5. Lib-internal `#[cfg(test)] mod tests` plus the
  HTML-invariant unit tests in `test_support` (76 tests total) all pass.

## [0.1.0] - TBD

Initial public preview release of Aozora Flavored Markdown.

### Added

#### Parse pipeline

- Seven-phase pure-functional lexer (`afm-lexer`) — sanitize / events /
  pair / classify / normalize / registry / validate — that resolves
  Aozora notations before the CommonMark parser runs (ADR-0008).
- Post-process AST splice in `afm-parser` — inline, block-leaf, and
  paired-container surgery that reinstates Aozora nodes after vanilla
  comrak parsing.
- Round-trip serializer — inverts the lexer via sentinel registry
  substitution in one O(n) byte sweep.

#### Aozora notations

- Ruby (`｜…《…》` and implicit-delimiter forms), including nested
  gaiji/annotation segments.
- Bouten (sideline emphasis), 11 variants including `《《…》》` and the
  `［＃「X」に傍点］` forward-reference form.
- Tate-chu-yoko (`［＃縦中横］`).
- Indentation — 字下げ / 地付き / 地寄せ / 複合字詰め.
- Headings — 大見出し / 中見出し / 小見出し / 窓見出し.
- Page breaks — 改丁 / 改ページ / 改見開き / 改段.
- Kunten (返り点) and 再読文字.
- Gaiji — JIS X 0213 / Unicode / 第3水準 reference styles, all
  compile-time resolved via a `phf::Map`.
- 割注 (inline split annotation) and container variants (罫囲み, etc.).
- Accent decomposition (`〔…〕`) with a 114-entry translation table.
- Illustration and section-break markers (挿絵 / 改段).

#### Encoding

- Transparent Shift_JIS decoding via `afm-encoding`.
- UTF-8 BOM sniff and strip.

#### CLI

- `afm render` / `afm check` subcommands.
- Global `--encoding {utf8,sjis}` and `--strict` flags.

### Quality gates

- 519 tests passing — unit + integration + snapshot + proptest.
- 96 % regions coverage CI floor.
- CommonMark 0.31.2 spec: 652 / 652 cases passing verbatim.
- GFM 0.29 spec passing verbatim.
- 17 k-work Aozora Bunko corpus sweep with four CI-gated invariants:
  I1 no panic, I2 no bare `［＃` leak, I3 round-trip fixed point,
  I4 HTML tag-balanced (ADR-0007).
- 『罪と罰』 (Aozora Bunko card 56656) Tier-A acceptance canary —
  panic-free rendering with zero unconsumed `［＃` markers.
- ~22-line diff against vendored comrak 0.52.0, well inside the 200-line
  budget from ADR-0001.
- `#![forbid(unsafe_code)]` workspace-wide; `dead_code = "deny"`;
  strict-code grep gate that rejects `#[allow(...)]`, nightly feature
  gates, and raw `println!` in library crates.

[Unreleased]: https://github.com/P4suta/afm/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/P4suta/afm/releases/tag/v0.1.0
