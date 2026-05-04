# Changelog

All notable changes to Aozora Flavored Markdown (afm) are recorded in
this file. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); the project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed (breaking)

- **`IrInline::Range` / `IrBlock::Range`** are now
  `{ start: Position, end: Position }` carrying 1-based line / column
  coordinates straight from comrak's `Sourcepos`. The previous
  `{ from: u32, to: u32 }` was a pseudo-byte offset
  (`(line-1)*1024 + (col-1)`) that silently broke under multi-byte
  CJK content. JS-side consumers (afm-obsidian's CodeMirror bridge)
  no longer need to redo UTF-8 byte arithmetic. TS contract on the
  consumer side must be updated to match.
- **`pub use aozora_pipeline::*_SENTINEL`** from `afm_markdown` is
  removed in favour of the afm-side wrapper module
  `afm_markdown::sentinels` (`INLINE` / `BLOCK_LEAF` / `BLOCK_OPEN` /
  `BLOCK_CLOSE`). The afm public API no longer names sibling-crate
  constants, so upstream renames surface in this module rather than
  breaking every consumer.
- **`Options<'c>` lifetime parameter** removed. `Options` now wraps
  `comrak::Options<'static>` and carries no caller-side lifetime,
  collapsing the 3-arg generic on every public entry point.

### Changed

- **`crates/afm-markdown/src/post_process.rs`** redesigned around
  `Cow<'_, str>` so the three secondary passes
  (`rebrand_aozora_classes_to_afm`, `wrap_orphan_brackets_in_place`,
  `balance_inline_tags_in_paragraphs`) borrow the previous pass'
  output on the common path and only allocate when their trigger
  pattern is present. Splicer Pass 1 is now the only mandatory
  allocation; Passes 2-4 are zero-allocation no-ops on well-formed
  input. The fully-fused 1-pass aho-corasick state-machine is
  documented as a follow-up in the module docstring; the Cow
  threading already removes the redundant *allocations* on the
  common path.
- **`splice_into`'s `<p>` matcher** now matches both `<p>` and
  `<p attr=…>` openings (taking the earliest of the two). Previously
  only `<p>` was matched, so source-line-anchor injection
  (`<p data-afm-source-line="N">`) could leak through the splicer
  unspliced. Fixes a long-standing asymmetry against
  `balance_inline_tags_in_paragraphs:127` which already handled both
  forms.
- **`source_line_anchors`** rewritten as `format_root_with_anchors`
  + `inject_anchor_into_first_open_tag`: comrak's `format_html` is
  invoked per top-level block and the anchor attribute is prepended
  to the first opening tag of each block's HTML chunk. The 226-line
  attribute-aware tag walker (with depth tracking, void-tag
  detection, attribute-value `>` handling) is gone; the new
  implementation is ~155 lines and self-contained.
- **`code_block_mask`** rewritten with `Cow<'_, str>`: when the
  source contains no fence markers (or already contains the mask
  char), the masking pass returns `Cow::Borrowed(input)` and skips
  allocation entirely. CRLF line breaks are now preserved through
  the mask/unmask round trip.
- **`ir.rs` (1318 L)** split into a `crates/afm-markdown/src/ir/`
  module: `types.rs` (public IR enum/struct definitions),
  `projection.rs` (pure conversion helpers and enum→string
  mappers), and `mod.rs` (the stateful walker + streaming builder).
- **`IrWalker` lifetime parameters** collapsed from three (`<'c, 'src,
  'a>`) to one (`<'src>`) plus per-method `<'a>` for comrak's
  invariant `Node` lifetime. The shared `SentinelCursor` now owns
  its `Vec<NodeRef>` rather than borrowing a slice, removing the
  slice-lifetime entirely from the walker's signature.
- **`crates/afm-markdown/src/sentinel_stream.rs`** (renamed from
  `sentinels.rs`) consolidates `walk_text_only_descendants` and
  `for_each_text_descendant` into a single
  `visit_text_leaves<F>(node, mode, f)` returning
  `core::ops::ControlFlow<()>` for early-exit. The two prior
  helpers are thin convenience wrappers around it.
- **`render_to_string` / `render_to_ir`** now delegate to a shared
  `drive_pipeline<F, T>` helper that owns the lex / parse / format
  / splice sequence. Each public entry point is ~5 lines of
  projection on top.

### Internal

- **`crates/afm-markdown-test-support/`** new sub-crate holds the
  test predicates and invariant helpers that previously lived in
  `afm-markdown::test_support` (1426 L behind `#[doc(hidden)] pub
  mod`). The hack is removed and the helpers are no longer part of
  `afm-markdown`'s public surface; the integration tests pull them
  in via `[dev-dependencies]` instead.
- **`saturating_u32`** centralised in `sentinel_stream` (was
  duplicated in `ir.rs` and `lib.rs`).
- **`AFM_CLASSES`** drift detection moved into the existing
  `css_class_contract.rs` integration test; the manual mirror in
  `test_support` carries a comment cross-referencing the sibling
  `aozora-render` source. (No build.rs codegen — the test is the
  drift detector.)
- Coverage measured at 97.86% regions across 283 tests; the 96%
  floor holds.

### Added

- **Aozora-side IR projection.** `afm_markdown::render_to_ir` and
  `render_blocks_to_ir` now emit every Aozora variant
  (`Ruby`, `DoubleRuby`, `Bouten`, `Tcy`, `Gaiji`, `Annotation`,
  `Container`, `PageBreak`, `SectionBreak`) into the typed
  `IrDocument`, replacing the v0.1 markdown-only walker. Heading
  hints (`［＃「X」は大見出し］`) promote their host paragraph to
  `IrBlock::Heading` directly. `IrInline::Image` is also added so
  CommonMark images survive the IR boundary.
- **`afm_markdown::ir::StreamingIrBuilder`.** Public stateful
  per-block IR builder that threads the sentinel-stream cursor
  across `walk_block` calls. afm-obsidian's chunked-cancellation
  path uses this to checkpoint between blocks without losing
  Aozora projection lockstep.
- **`crates/afm-markdown/src/sentinels.rs`.** New shared module
  that owns `BlockSentinelKind`, `is_sentinel_char` (subtraction-
  based fast check), `sole_block_sentinel`,
  `flatten_registry_in_source_order`, and `SentinelCursor`
  (peek / next / advance / position primitive). Both the HTML
  splicer and the IR builder consume from this single source of
  truth.
- **ADR-0011 — brand boundary CSS class rewrite.** Codifies the
  decision to keep the `aozora-*` → `afm-*` HTML rewrite on the
  afm side rather than parameterising upstream `aozora-render`,
  preserving the one-way `afm → aozora` dependency direction.
- **`cargo xtask upstream-sync <tag>`** is now implemented as a
  pure tree-replace: shallow-clones the upstream comrak tag, drops
  the old vendored tree, copies the new source over, and updates
  `COMRAK_SHA`. The `afm-side` metadata (`COMRAK_SHA`,
  `UPSTREAM_DIFF.md`) is preserved across the wipe.

### Changed (breaking)

- **`IrInline::DoubleRuby`** drops the always-empty `outer` and
  `inner` string fields. The shape is now
  `{ base: Vec<Self>, range }` matching upstream's `DoubleRuby`
  payload exactly.
- **`RenderedBlock.ir`** is now `Vec<IrBlock>` rather than a
  single `IrBlock`. This removes the `ThematicBreak` placeholder
  hack for comrak constructs without a v0.2 IR projection
  (definition list, footnote ref, raw HTML) and lets paired-
  container drains carry through the streaming boundary.
- **`AnnotationKind::Unknown`** projects to
  `Some("unknown")` in `IrInline::Annotation::resolved` instead
  of `None`. Future `#[non_exhaustive]` variants of
  `AnnotationKind` upstream will surface as `None`, so consumers
  can distinguish "the parser tried and gave up" from "afm
  doesn't know about this kind yet".
- **`pub use comrak::Options as ComrakOptions`** removed from
  the public surface. Consumers who tweak comrak's options
  directly should import comrak themselves; the afm public API
  no longer pins comrak's version into its surface.

### Changed

- **`afm-wasm` diagnostic projection** now uses
  `Diagnostic::severity` / `source` / `code` plus the `Display`
  impl, replacing the hardcoded `"info"` level and `"{d:?}"`
  debug-format message. Wire shape is
  `{ level, source, code, message }`.
- **`afm_markdown::post_process`** now consumes the shared
  `SentinelCursor` instead of carrying its own cursor fields.
- **`UPSTREAM_DIFF_BUDGET_LINES`** in `xtask` lowered from 200
  to 0, matching ADR-0001 v0.2.4.

### Removed

- **`xtask` deferred sub-commands** (`corpus-refresh`, `corpus-test`,
  and the `deferred()` helper) — moved to the sibling `aozora`
  repo per ADR-0010.
- **`aozora-corpus`** dropped from `[workspace.dependencies]`
  (not used by any member crate after ADR-0010).
- **`afm_markdown::ir::walk_block_public`** removed in favour of
  `StreamingIrBuilder` so multi-block streaming consumers can't
  accidentally restart the cursor between blocks.

### Documentation

- **afm-book** refreshed top-to-bottom: `library.md` rewritten
  with current `afm_markdown` API examples (3-tier:
  `render_to_string`, `render_to_ir`, `render_blocks_to_ir`,
  plus `serialize`); `arch/pipeline.md` replaced with the
  current 3-layer + shared-cursor architecture; `arch/adr.md`
  expanded to the full 0001-0011 set with current statuses;
  `ref/api.md` re-targeted at `afm_markdown` / `afm_wasm` and
  the sibling `aozora-*` crates.
- **CONTRIBUTING.md** rewritten around the post-v0.2.0 glue-
  layer responsibility. The 5-step "How to add an invariant"
  flow is now afm-markdown-internal; new 青空文庫 notations
  redirect to the sibling repo.
- **README.md / README.ja.md / SECURITY.md / PR template** —
  stale `afm-parser` / `afm-lexer` / `afm-syntax` / `afm-encoding`
  references and the obsolete `200-line` budget removed.
- **ADR-0003** (afm-parser architecture) and **ADR-0005**
  (paired-block container hook) statuses updated to
  `Superseded by ADR-0010` / `Superseded by ADR-0008` with
  v0.2.0 / v0.2.4 historical context appended.
- **Stale code comments** in `afm_markdown::lib`,
  `afm_markdown::examples::{render-utf8,render-sjis}`, and
  `xtask::spec_refresh` updated to match current crate names.

### Internal

- Coverage measured at 97.23% regions across 273 tests; the 96%
  floor holds. New unit tests pin every non-exhaustive enum
  match arm (`bouten_kind_str`, `section_kind_subtype`,
  `container_subtype`, `container_indent_level`,
  `annotation_kind_resolved`, `bouten_position_str`) so future
  upstream additions surface immediately.
- `IrWalker` uses move semantics for `OpenContainer` children
  (no clone at close), and `ParaScan` runs a single descent over
  each paragraph to compute `total_sentinels` / `first_heading_hint`
  in one pass.

## [0.3.0] - 2026-04-30

Major release. Tracks aozora `0.2.6` (released same day) and locks in
the **brand boundary** between `aozora-*` (pure 青空文庫記法) and
`afm-*` (Aozora Flavored Markdown).

### Changed (breaking)

- **Bumped pinned `aozora-*` crates from v0.2.5 → v0.2.6.** Picks up
  upstream PR #4 (afm-* → aozora-* class prefix flip + gaiji
  `data-codepoint` / `data-description` attrs + wasm-pack pipe fix),
  PR #5 (docs overhaul / driver build integration / ADR cleanup),
  PR #6 (pymodule rename for maturin).
- **Brand boundary in `post_process::splice_aozora_html`.** The
  upstream `aozora-render` crate now emits `aozora-*` CSS classes;
  afm-markdown's HTML output continues to carry the `afm-*` brand
  (Aozora Flavored Markdown). A new
  `rebrand_aozora_classes_to_afm` post-process pass rewrites every
  `aozora-*` class token in the spliced HTML to its `afm-*`
  counterpart. Touches only `class="..."` attribute values; data-*
  attributes, link targets and text bodies are preserved verbatim.

### Internal

- `aozora_parity` test runner switched to a stem-based histogram
  (`class_stem_histogram(html, prefix)`) so the differential against
  `aozora-render` compares the family of recognisers fired, not the
  brand prefix.
- Coverage measured at 98.77 % regions across 179 tests, no ignored
  cases, all eleven integration tests + four examples building
  against the new public API.

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
