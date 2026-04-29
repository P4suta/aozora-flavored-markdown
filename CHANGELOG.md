# Changelog

All notable changes to Aozora Flavored Markdown (afm) are recorded in
this file. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); the project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.6] - 2026-04-30

Closes every v0.2.5 follow-up by **resolving** them (no `#[ignore]`, no
floor lowering). 179/179 tests pass with zero gates; coverage is back
above the 96 % regions floor. The `block_structure_interaction::fenced
_code_block_*` test that v0.2.5 marked as a known limitation is now a
true assertion.

### Added

- **CommonMark code-block-aware lex pre-pass.** New
  `code_block_mask` module hides 青空文庫 trigger characters
  (`｜《》［］※〔〕「」`) inside fenced code blocks before
  `aozora-lex` sees the source, then unmasks them in the rendered
  HTML. Aozora markup inside ` ``` ` / `~~~` fences now flows through
  to `<pre><code>` literally — the formerly `#[ignore]`d
  `fenced_code_block_preserves_aozora_markup_as_code` is unblocked.
- **Defensive Tier-A guard** in `post_process::splice_aozora_html`:
  any bare `［＃…］` that the upstream lexer fails to claim (e.g.
  empty annotation `［＃］` nested inside a baseless ruby pair `《》`,
  which `aozora-lex` Phase 3's replay path drops) is auto-wrapped in
  an `afm-annotation` hidden span. The Tier-A canary now holds for
  every input the property tests can generate, including the three
  pathological seeds (`［＃`, `］［＃`, `《［＃］》`) that v0.2.5
  could not satisfy.
- **lib + post_process unit tests** pinning every formerly-uncovered
  region: `Options::gfm_only`, the `contains_bare_bracket` helper,
  malformed `</p>` recovery, exhausted-registry block sentinel,
  block-sentinel-inside-inline drop, HeadingHint target HTML escape.

### Changed

- **Coverage gate restored to 96 %.** `_COV_FLOOR = 96` (was 93 in
  v0.2.5), with `test_support.rs` excluded from the measurement
  because it is `#[doc(hidden)] pub mod` test-helper code, not
  production. Production coverage measures **99.26 %** across
  `lib.rs` (100 %), `html.rs` (100 %), `post_process.rs` (98.6 %),
  and `code_block_mask.rs` (98.97 %).
- **CLAUDE.md** Open-follow-ups section reframed: Aozora-only
  fixtures (`spec-aozora` / `spec-golden-56656` / `corpus-sweep`)
  now correctly point to the sibling `P4suta/aozora` repo (they
  moved there at v0.2.0 — afm only keeps the CommonMark/GFM spec
  runners).
- **ADR-0001** carries a v0.2.4 status update documenting the diff
  budget collapse (200 → 0).
- **`.claude/settings.local.json`** added to `.gitignore` per the
  per-project Claude Code convention.

### Internal

- aozora-tools (225 tests + ADRs) and afm-epub (placeholder) verified
  unchanged after this release: the only modifications live in
  afm-markdown's own surface plus tooling, so the sibling repos pass
  unchanged.

## [0.2.5] - 2026-04-30

Closes the v0.2.5 follow-up list from v0.2.4. Every integration test
and example is now back on the new public API; `just test` runs the
full 159-test suite.

### Added

- **Heading-hint promotion.** A paragraph carrying a `HeadingHint`
  inline sentinel (`［＃「X」は大見出し／中見出し／小見出し］`) now
  renders as `<h{level}>{target}</h{level}>`. `post_process` peeks at
  the registry from inside the paragraph, rewrites the wrapper, and
  consumes the hint's siblings so indent / annotation classes don't
  leak into the heading body.
- **Stack-balanced container splice.** `BlockOpen` paragraphs push
  onto a `Vec<ContainerKind>`; `BlockClose` paragraphs pop. Open-less
  closes are silently dropped, and any container left open at end-of-
  document is auto-closed so the Tier-D HTML tag-balance invariant
  holds for malformed inputs too.
- **Family-suffix CSS class recognition.** `is_recognised_afm_class`
  now accepts any `<base>-<suffix>` where `<base>` is in
  `AFM_CLASSES`, covering both numeric modifiers (`afm-indent-2`,
  `afm-container-indent-3`) and slug modifiers (`afm-section-break-
  choho`, `afm-bouten-goma`-suffixed forms) without expanding the
  pinned list per variant.

### Re-enabled

- All 11 integration tests are back in CI:
  `commonmark_spec` (652 examples), `gfm_spec` (extension-tagged 0.29
  spec), `css_class_contract`, `html_well_formed`,
  `block_structure_interaction` (1 case `#[ignore]`d — fenced code
  block contents still need a CommonMark-aware lex skip),
  `paired_container`, `heading_promotion`, `property_html_shape`,
  `property_heading_integrity`, `post_process_invariants` (redrafted
  against HTML; the AST helpers it used are gone), `aozora_parity`
  (redrafted around `aozora_lex` + `aozora_render`).

### Internal

- `splice_aozora_html` is now paragraph-aware *and* still inline-aware
  outside `<p>...</p>` boundaries (so headings, list items,
  blockquotes, table cells keep getting their inline sentinels
  resolved). The two-stage loop is documented in the module header.
- `SpliceState` replaces the previous `IntoIter` plumbing so
  `process_paragraph` can `peek()` ahead before deciding between
  heading promotion and a regular inline pass.

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
