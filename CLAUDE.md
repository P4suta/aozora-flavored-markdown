# afm — Claude Code project guide

Opening note for any Claude Code session that enters this repo: read this file
first. It is the shortest path to productive work.

## What this is

**aozora-flavored-markdown (afm)**: a Rust fork of
[comrak 0.52.0](https://github.com/kivikakk/comrak) that layers Aozora Bunko
(青空文庫) typography — ruby, bouten, 縦中横, `［＃...］` annotations, gaiji,
accent decomposition — on top of CommonMark + GFM. Ships as a single binary
and an embeddable library.

Hard guarantees:
- **100 % CommonMark 0.31.2 + GFM compatibility** (verbatim spec tests pass,
  652 + GFM cases).
- **Aozora Bunko compatibility target**: every notation at
  <https://www.aozora.gr.jp/annotation/> parses; the 『罪と罰』 fixture is the
  day-1 golden (Tier A = no bare `［＃` leaks in the HTML output).
- **Single binary**, no runtime process dependencies.
- **Pure-functional parse pipeline** (ADR-0008): zero parse-time hooks in
  upstream comrak; Aozora recognition lives entirely in `afm-lexer` + an
  AST-splice pass in `afm-parser`.
- **TDD with C1 branch coverage** on the Rust crates, layered in from every
  angle: unit tests per phase, integration fixtures, property tests,
  cross-layer invariants (Tier-A canary, HTML-escape, post-process surgery),
  17 k-work corpus sweep, end-to-end CLI tests.

## Architecture (ADR-0008)

The parse pipeline is a three-step funnel — Aozora is fully resolved before
comrak runs, and spliced back into the AST afterwards. Upstream comrak has
**no Aozora parse hooks** and exactly one render-side `fn` pointer seam (with
a matching serialize-side hook deliberately *not* taken — the serializer
inverts the pipeline directly).

```text
source (UTF-8 or SJIS)
   │
   ▼ afm-encoding::decode   (if SJIS)
   │
┌──┴───────────── afm-lexer::lex  (pure function, 7 phases) ──────────────┐
│ 0 sanitize   BOM / CRLF→LF / 〔…〕 accent / PUA collision scan           │
│ 1 events     linear tokenise of Aozora trigger glyphs                    │
│ 2 pair       balanced-stack pairing (brackets / quotes / ruby / 〔〕)    │
│ 3 classify   per-shape → AozoraNode + ContainerKind (with               │
│              Annotation{Unknown} fallback so every `［＃…］` is claimed)  │
│ 4 normalize  replace each Aozora span with a PUA sentinel                │
│              (U+E001 inline / U+E002 block-leaf /                         │
│               U+E003 block-open / U+E004 block-close, block sentinels    │
│               padded with `\n\n` so comrak reads them as standalone p)   │
│ 5 registry   binary-search lookup from sentinel position to node         │
│ 6 validate   V1-V3 invariants (no `［＃` leak, every PUA recorded, …)    │
│                                                                          │
│ Output: LexOutput { normalized, registry, diagnostics }                  │
└───┬──────────────────────────────────────────────────────────────────────┘
    │
    ▼  comrak::parse_document  (vanilla CommonMark + GFM; no Aozora hooks)
    │
┌───┴────── afm-parser::post_process  (AST surgery) ──────────────────────┐
│ splice_inline            : every Text node with U+E001 is split and      │
│                             the sentinel replaced with Aozora(...)       │
│ splice_block_leaf        : every Paragraph whose single child is U+E002  │
│                             is replaced with the Aozora block node       │
│ splice_paired_container  : stack-walk over U+E003/U+E004 sentinel        │
│                             paragraphs, wrapping siblings into an        │
│                             AozoraNode::Container block node             │
└───┬──────────────────────────────────────────────────────────────────────┘
    │
    ▼  comrak HTML renderer  (NodeValue::Aozora(_) arm → afm render fn,
    │                         entering flag for Container open/close)
    │
    ▼
   HTML
```

The full-round-trip path reuses the lexer's output as its inverse input:

```text
ParseResult { root, diagnostics, artifacts: { normalized, registry } }
                                            │
                                            ▼  afm-parser::serialize
                                            │   (single O(n) byte sweep;
                                            │    match_indices finds each
                                            │    PUA sentinel; registry
                                            │    substitutes the original
                                            │    afm markup back in)
                                            ▼
                                        afm text
```

### Crates

| Crate | Responsibility |
|---|---|
| `afm-syntax` | `AozoraNode` AST (`Ruby` / `Bouten` / `TateChuYoko` / `Gaiji` / `Kaeriten` / `Annotation` / `PageBreak` / `SectionBreak` / `Indent` / `AlignEnd` / `Sashie` / `AozoraHeading` / `DoubleRuby` / `Container` / …), `Content` / `Segment`, 114-entry accent table, `ContainerKind`, `BoutenKind` (11 variants), `BoutenPosition`. |
| `afm-lexer` | 7-phase pure-functional Aozora recogniser. `lex(source) → LexOutput`. The project's core parsing engine. |
| `afm-parser` | `parse()` = lex + comrak + `post_process` splice, returning `ParseResult { root, diagnostics, artifacts }`. `serialize(&ParseResult) → String` inverts the pipeline. Owns the HTML renderer under `aozora/html.rs` (registered as a `fn` pointer on `comrak::Options::extension::render_aozora`). |
| `afm-encoding` | Shift_JIS decoding, UTF-8 BOM sniff, Gaiji resolution via a compile-time `phf::Map` keyed by mencode (`第3水準…` / `U+XXXX`) with description fallback. |
| `afm-cli` | `afm` binary — `render` / `check` subcommands with `--encoding {utf8,sjis}` and global `--strict` (fail on any lexer diagnostic). |
| `afm-corpus` | Corpus test sources (`InMemory` / `Vendored` / `Filesystem`) for the 17 k-work sweep. |
| `afm-book` | mdbook documentation site with `theme/afm-horizontal.css` and `theme/afm-vertical.css` covering every renderer-emitted class. |
| `xtask` | dev automation (spec-refresh, corpus-sweep plumbing, new-adr, …). |
| `upstream/comrak/` | vendored fork v0.52.0 — **200-line diff budget** (ADR-0001). One `NodeValue::Aozora` arm, one render-side `fn` pointer (with an `entering` flag for container enter/exit), zero parse hooks. |

## Architecture Decision Records

Read these before touching anything the ADR governs. `docs/adr/` on disk.

- **ADR-0001** — fork comrak and vendor in-tree; 200-line diff budget enforced.
- **ADR-0002** — Docker-only execution: every tool runs via `docker compose run` through `Justfile` targets, never directly on the host.
- **ADR-0003** — initial afm-parser architecture (`Arc<dyn AozoraExtension + 'c>` trait object on `Extension.aozora`). **Parse-phase portion superseded by ADR-0008**; render-phase dispatch survives as a naked `fn` pointer after D2.
- **ADR-0004** — accent decomposition inside `〔...〕`. Originally a preparse pass; **folded into `afm-lexer::phase0_sanitize` by E2/C5b**.
- **ADR-0005** — paired block annotation container hook. **Superseded by ADR-0008** (paired-container handling will live in `post_process`, not as an upstream hook).
- **ADR-0006** — lint profile policy & scope discipline: `[workspace.lints]` is the single source of truth; `-W clippy::<group>` flags are banned on the `just clippy` command line because they silently override per-lint carve-outs.
- **ADR-0007** — corpus sweep strategy: `afm-corpus` + I1/I2/I3/I4/I5 invariants over 17 k real Aozora works as a regression floor.
- **ADR-0008** — **zero-parser-hook Aozora-first pipeline**. The architecture above: Aozora recognition happens in `afm-lexer` before comrak, is spliced back into the AST afterwards, and leaves comrak parse phase untouched. **Current architecture.**

## Development environment

Docker is the only accepted execution surface. Host toolchain invocations
(`cargo test`, `mdbook build`, `playwright`, …) are forbidden in automation.
`just` runs on the host and shells through `docker compose run`.

```
just build                # cargo build --workspace --all-targets
just test                 # cargo nextest run --workspace
just lint                 # fmt-check + clippy pedantic+nursery + typos + strict-code grep
just coverage             # llvm-cov branch
just spec-commonmark      # CommonMark 0.31.2 spec (652 cases)
just spec-gfm             # GFM spec
just spec-aozora          # hand-written aozora fixtures
just spec-golden-56656    # 『罪と罰』 Tier A acceptance gate
just corpus-sweep         # 17 k-work invariant sweep (ADR-0007)
just upstream-diff        # enforce 200-line budget vs comrak v0.52.0 (xtask pending)
just book-serve           # mdbook preview on http://localhost:3000
just ci                   # replicate the full CI pipeline locally
just adr '<title>'        # scaffold a new ADR via xtask
just upstream-sync <tag>  # sync vendored comrak to a new tag

just watch [JOB]          # bacon watcher (default job: check). Keybinds:
                          #   t=test c=clippy d=doc f=failing-only esc=back
                          #   q=quit  Ctrl-J=list jobs
just hooks                # install lefthook git hooks (pre-commit fmt+re-stage,
                          # clippy, typos, upstream-diff; pre-push test+deny;
                          # commit-msg Conventional Commits)
just hooks-uninstall      # remove them
just sccache-stats        # sccache hit/miss ratio + cache size
just sccache-zero         # reset counters before a measurement window
```

Bootstrap steps for a fresh clone:

```
docker compose build dev      # ~5 min first time, cached after
jj git init --colocate        # if jj isn't already initialised
just hooks                    # wire lefthook pre-commit / commit-msg / pre-push
just test                     # confirm green
```

## Host tools vs container tools

Host-installed modern CLIs (`rg`, `fd`, `sd`, `jq`, `yq`, `bat`, `eza`, `just`,
`jj`, `atuin`, `starship`, `chezmoi`, …) are for exploratory work:

- `rg '［＃' upstream/comrak` — scan vendored comrak for annotation handling.
- `fd -e json spec/` — list spec fixtures.
- `jj log --limit 20` — review recent history through the jj lens.

Every build/test target must go through `just` (and therefore through Docker).
The host/container split is enforced by `ADR-0002`.

## Version control

Repo is **colocated jj + git**: the working copy is managed by jj (via
`jj commit`, `jj log`, `jj rebase`), and each operation reflects into `.git/`
so GitHub Actions and `git log` both keep working. Prefer `jj describe -m` +
`jj bookmark move main --to @` + `jj new` for the commit-advance-new cycle;
fall back to `git commit` only when scripting or when a tool requires it.

## TDD flow for new features

Aozora recognition lives in the lexer, not in a comrak hook. A feature
lands in this order:

1. **Spec fixture** — add a `spec/aozora/cases/<kind>.json` case with the input +
   expected HTML. Case names document the shape being claimed.
2. **AST variant** — if the notation needs a new `AozoraNode` shape, add it to
   `crates/afm-syntax/src/lib.rs` with `#[non_exhaustive]`. If the shape is
   block-level and wraps block children, use `AozoraNode::Container`
   (children live in the comrak AST's sibling chain, not embedded in the
   node).
3. **Lexer classifier (red)** — add a unit test to
   `afm-lexer/src/phase3_classify.rs::tests` asserting the new `AozoraNode`
   is emitted for a sample source. Every well-formed `［＃…］` that the
   classifier doesn't claim falls through to `Annotation{Unknown}` via the
   built-in catch-all — there is no "silent plain text" path.
4. **Lexer classifier (green)** — implement the recogniser in
   `afm-lexer/src/phase3_classify.rs`. For forward-reference shapes (bouten /
   TCY) the helper `forward_target_is_preceded` handles the target-exists
   check. Multi-quote shapes reuse `extract_forward_quote_targets`.
5. **post_process splice if needed** — only paired-container shapes that
   reparent siblings touch `afm-parser/src/post_process.rs`. Inline and
   block-leaf sentinels are already handled generically.
6. **Renderer** — add the branch to `crates/afm-parser/src/aozora/html.rs`.
   All user content must pass through `escape_text` (the HTML-escape
   invariant suite in `tests/html_escape_invariants.rs` catches regressions
   here). Container variants honour the `entering: bool` flag and emit the
   open tag on entry, close tag on exit.
7. **Serializer** — if the variant emits a new afm markup shape, add the
   emitter to `crates/afm-parser/src/serialize.rs`. The sweep's I3
   (round-trip fixed point) will fail until every shape re-parses cleanly.
8. **CSS themes** — add a rule to both `crates/afm-book/theme/
   afm-horizontal.css` and `.../afm-vertical.css`, and append the class
   token(s) to the pinned list in `tests/css_class_contract.rs`.
9. **Cross-layer invariants** — if the new shape has non-trivial interactions
   with CommonMark block structures, add cases to
   `tests/block_structure_interaction.rs`. If it's a new opcode for
   `post_process`, add cases to `tests/post_process_invariants.rs` (inc. the
   proptest).
10. **Verify** — `just lint && just test && just spec-golden-56656`.

No commit lands without the full gate passing locally.

## Where to find what

```text
crates/afm-syntax/src/
  lib.rs               # AozoraNode + Ruby / Bouten / TCY / Gaiji / Kaeriten /
                       # Annotation / Content / Segment / SectionKind / ...
  extension.rs         # ContainerKind (AozoraExtension trait was deleted in D2)
  accent.rs            # 114-entry accent decomposition table

crates/afm-lexer/src/
  lib.rs               # pub fn lex() — 7-phase pipeline entry
  phase0_sanitize.rs   # BOM / CRLF→LF / 〔…〕 accent decomposition / PUA scan
  phase1_events.rs     # linear tokenise of Aozora trigger glyphs
  phase2_pair.rs       # balanced-stack bracket / ruby / quote pairing
  phase3_classify.rs   # per-shape Aozora classification + Annotation{Unknown}
                       # catch-all; forward-ref target-exists check; indent
                       # zero-digit reject
  phase4_normalize.rs  # PUA sentinel substitution (\n\n padding for blocks)
  phase5_registry.rs   # binary-search lookup API over the registry
  phase6_validate.rs   # V1-V3 structural invariants
  token.rs             # Token / TriggerKind
  diagnostic.rs        # Diagnostic enum + miette glue

crates/afm-parser/src/
  lib.rs               # pub fn parse() → ParseResult { root, diagnostics,
                       #                                 artifacts }
  html.rs              # render_to_string convenience entry
  serialize.rs         # pub fn serialize(&ParseResult) → String
                       # (inverts lex via registry substitution)
  post_process.rs      # splice_inline / splice_block_leaf /
                       # splice_paired_container AST surgery
  test_support.rs      # test helpers (#[doc(hidden)] pub)
  aozora/
    mod.rs             # render-side submodule declarations
    html.rs            # AozoraNode → HTML (registered as render_aozora fn)
    bouten.rs          # kind_slug + position_slug CSS-class tables

crates/afm-parser/tests/
  golden_56656.rs                 # 罪と罰 Tier-A floor + annotation census
  aozora_spec.rs                  # hand-written fixtures runner
  commonmark_spec.rs              # CommonMark 0.31.2 (652 cases)
  gfm_spec.rs                     # GFM 0.29 spec
  html_escape_invariants.rs       # XSS / 5 OWASP HTML escapes
  post_process_invariants.rs      # AST surgery + determinism + proptest
  block_structure_interaction.rs  # Aozora × CommonMark block shapes
  html_well_formed.rs             # balanced-tag validator + fixtures
  paired_container.rs             # Container open/close wrap
  ruby_segments.rs                # nested gaiji/annotation inside ruby
  property_ruby.rs                # proptest on ruby round-trip
  css_class_contract.rs           # pinned renderer classes ↔ theme CSS
  forward_reference_regression.rs
  long_paragraph_regression.rs
  corpus_sweep.rs                 # 17 k-work I1/I2/I3/I4/I5 sweep
  common/mod.rs                   # shared balanced-HTML validator

crates/afm-cli/
  src/main.rs                     # clap CLI: afm render / afm check
                                  #          + global --strict / --encoding
  tests/cli_integration.rs        # binary-level integration

crates/afm-encoding/src/
  lib.rs                          # decode_sjis / has_utf8_bom
  gaiji.rs                        # phf::Map mencode → char + U+XXXX parser

crates/afm-book/
  theme/afm-horizontal.css        # left-to-right writing-mode theme
  theme/afm-vertical.css          # vertical-rl (tategaki) theme

spec/
  aozora/fixtures/56656/          # 罪と罰 SJIS + UTF-8 + golden HTML
  aozora/cases/*.json             # hand-written annotation cases
  commonmark-0.31.2.json          # vendored spec
  gfm-0.29-gfm.json               # vendored spec

docs/
  adr/                            # 0001 … 0008
  specs/aozora/                   # vendored Aozora Bunko annotation spec pages
  plan.md                         # milestone plan snapshot

upstream/comrak/                  # v0.52.0 + ADR-0008-minimal surface
  COMRAK_SHA                      # pinned upstream sha
  UPSTREAM_DIFF.md                # diff-budget policy
```

## DO NOT

- **Do not modify `upstream/comrak/` without an ADR.** The 200-line diff
  budget and quarterly sync strategy depend on every change being an explicit
  hook, not ad-hoc logic.
- **Do not re-introduce parse-time hooks in comrak.** ADR-0008 is the whole
  point: parse phase is vanilla CommonMark+GFM over PUA-sentinel text, and
  the only surviving seam is the render-side `fn` pointer.
- **Do not put recogniser / classifier logic in `afm-parser/src/aozora/`.**
  That directory is render-only post-D1. New Aozora shapes go into
  `afm-lexer/src/phase3_classify.rs`.
- **Do not bypass the Tier-A canary.** No bare `［＃` may appear in the HTML
  outside the `afm-annotation` hidden wrapper. The canary is asserted by
  `golden_56656`, `post_process_invariants` (proptest), and
  `block_structure_interaction` simultaneously — a single layer catching it
  is not enough.
- **Do not run cargo / mdbook / node directly on the host.** `just` +
  Docker is the only sanctioned path (ADR-0002).
- **Do not suppress warnings** (`#[allow(...)]`, `continue-on-error`, etc.).
  `dead_code = "deny"` is workspace-level; a warning is a real signal.
  See `feedback_no_warning_suppression`.
- **Do not guess at Aozora encoding edge cases.** Read
  `docs/specs/aozora/*.html` or the live page before patching. See
  `feedback_read_aozora_spec_first`.
- **Do not pin dependency versions from memory.** Verify against
  crates.io / npm / GitHub Releases at decision time.

## At a glance

- **Workspace tests**: 519 passing (lexer unit tests + parser integration +
  CLI integration + proptest-driven invariants + spec-conformance runners).
- **Coverage**: floor enforced at 96 % regions
  (`_COV_FLOOR` in Justfile); measured 96.07 %.
- **Lint**: workspace-level clippy with `dead_code = "deny"`;
  `just strict-code` forbids `#[allow]` / `#[feature]` /
  `unsafe_code` / bare `TODO` / untracked `println!` in libraries.
- **Corpus sweep**: five invariants (`crates/afm-parser/tests/corpus_sweep.rs`),
  I1 (no panic), I2 (no bare `［＃` leak), I3 (`serialize ∘ parse` fixed point),
  I4 (HTML tag-balanced) are hard-gated; I5 (SJIS decode stable) is report-only.
  Each hard gate has an `AFM_CORPUS_*_BUDGET` env-var escape hatch.
- **Golden**: 罪と罰 fixture passes Tier-A (no bare bracket leaks) with
  a ≥ 400 bracket-sourced-annotation census floor.
- **Upstream comrak diff**: ~22 lines out of the ADR-0001 200-line budget —
  one `NodeValue::Aozora` arm, one render `fn` pointer (with `entering`).
- **Public API**: `parse(arena, input, &options) → ParseResult<'a>` carrying
  `root: &AstNode<'a>`, `diagnostics: Vec<Diagnostic>`, and
  `artifacts: Option<ParseArtifacts>` (the lexer's normalized text +
  registry, used by `serialize(&ParseResult)`).
