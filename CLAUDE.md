# afm — Claude Code project guide

Opening note for any Claude Code session that enters this repo: read this
file first. It is the shortest path to productive work.

## What this is

**aozora-flavored-markdown (afm)**: a Rust integration layer that composes
[comrak 0.52.0](https://github.com/kivikakk/comrak) (vanilla, vendored)
with the borrowed-AST 青空文庫記法 parser shipped in
[`P4suta/aozora`](https://github.com/P4suta/aozora). afm produces HTML
where CommonMark + GFM and Aozora Bunko typography (ruby, bouten, 縦中横,
`［＃...］` annotations, gaiji, accent decomposition) coexist correctly.
Ships as the `afm` binary and an embeddable library.

Hard guarantees:
- **100 % CommonMark 0.31.2 + GFM compatibility** — comrak is verbatim
  v0.52.0; the spec runners are inherited unmodified.
- **Aozora Bunko compatibility** — every notation at
  <https://www.aozora.gr.jp/annotation/> that the upstream `aozora`
  parser recognises is rendered correctly. Tier-A invariant (no bare
  `［＃` leak in HTML output) is preserved end-to-end.
- **Zero comrak modifications.** `upstream/comrak/` stays bit-for-bit
  identical to upstream v0.52.0 (ADR-0001 budget = 0).
- **Single binary, no runtime process dependencies.**

## Architecture

```text
source (UTF-8 or SJIS)
   │
   ▼ aozora_encoding::decode_sjis  (if SJIS)
   │
┌──┴── aozora_pipeline::lex_into_arena (borrowed-AST) ────────────────────┐
│ Phase 0  sanitize   BOM / CRLF→LF / 〔…〕 accent / PUA collision scan   │
│ Phase 1  events     SIMD trigger-byte tokenise (aozora-scan)            │
│ Phase 2  pair       balanced-stack bracket / ruby / quote pairing        │
│ Phase 3  classify   borrowed AozoraNode<'arena> + ContainerKind, with   │
│                     Annotation{Unknown} catch-all so every `［＃…］`    │
│                     is claimed                                           │
│ Fused walk: emits PUA-rewritten text into the arena +                    │
│ builds the four EytzingerMap registry tables                             │
│                                                                          │
│ Output: BorrowedLexOutput<'arena> { normalized, registry, diagnostics }  │
└───┬──────────────────────────────────────────────────────────────────────┘
    │
    ▼  comrak::parse_document   (vanilla CommonMark + GFM; sentinel chars
    │                            U+E001..U+E004 flow through as plain
    │                            UTF-8 — they are not in `<>&"'` escape
    │                            set)
    │
    ▼  comrak::format_html       (vanilla; sentinels survive into output)
    │
┌───┴── afm_markdown::post_process::splice_aozora_html ────────────────────┐
│ Pre-flatten the registry into Vec<NodeRef<'a>> in source order; then     │
│ scan the comrak HTML once:                                               │
│                                                                          │
│   • `<p>U+E002</p>` paragraph  → render_node(Leaf, true)                 │
│   • `<p>U+E003</p>` paragraph  → render_node(Container, true)            │
│   • `<p>U+E004</p>` paragraph  → render_node(Container, false)           │
│   • U+E001 inside paragraph    → render_node(Inline, true)               │
│                                                                          │
│ aozora_render::render_node::render is the per-node HTML writer; it       │
│ owns every `afm-*` CSS class emitted.                                    │
└───┬──────────────────────────────────────────────────────────────────────┘
    │
    ▼
   HTML
```

The serializer path is just a delegate:

```text
input
   │
   ▼ aozora_pipeline::lex_into_arena
   │
   ▼ aozora_render::serialize::serialize  (round-trips byte-equal markup)
   │
   ▼
afm-source text
```

### Workspace crates

| Crate | Responsibility |
|---|---|
| `afm-markdown` | Glue layer. Owns `render_to_string(input, &options) -> Rendered { html, diagnostics }`, `serialize(input) -> String`, the `html::render_to_string(input) -> String` shim, and `post_process::splice_aozora_html` (HTML sentinel substitution). |
| `afm-cli` | `afm` binary — `render` / `check` subcommands with `--encoding {utf8,sjis}` and global `--strict` (fail on any aozora-side diagnostic). |
| `afm-book` | mdbook documentation site with `theme/afm-horizontal.css` and `theme/afm-vertical.css` covering every `afm-*` class `aozora-render` emits. Not a Rust crate. |
| `xtask` | Dev automation (spec-refresh, new-adr, …). |
| `upstream/comrak/` | Vendored fork v0.52.0 — **0-line diff budget** (ADR-0001). |

External (sibling repo) crates pulled in via git deps from
[`P4suta/aozora`](https://github.com/P4suta/aozora):

- `aozora-syntax` — `borrowed::AozoraNode<'a>` + bumpalo `Arena` + `Container` / `ContainerKind` / `BoutenKind` / `BoutenPosition` / `AozoraHeadingKind` / `Indent` / `AlignEnd` / `SectionKind` / 114-entry accent table.
- `aozora-pipeline` — `lex_into_arena(src, &arena) -> BorrowedLexOutput<'a>` plus the four PUA sentinel constants and `aozora_spec` re-exports.
- `aozora-render` — `html::render_to_string` / `render_into`, `serialize::serialize` / `serialize_into`, and `render_node::render` (per-node HTML writer).
- `aozora-encoding` — Shift_JIS decoding + gaiji resolution.
- `aozora-spec` — `Diagnostic`, `Span`, sentinel constants, slug tables.
- `aozora-proptest` (dev-only) — proptest generators, default config.
- `aozora-corpus` (dev-only, when needed) — corpus source abstraction.

The `[workspace.dependencies]` table in `Cargo.toml` is the single source
of truth for the aozora dependency configuration.

## Architecture Decision Records

Read these before touching what they govern.

- **ADR-0001** — fork comrak and vendor in-tree. Diff budget = 0 lines;
  upstream-sync is a pure tree replace.
- **ADR-0002** — Docker-only execution: every cargo / mdbook / playwright
  invocation runs through `docker compose run` via `Justfile` targets.
  Never run cargo on the host.
- **ADR-0009** — authoring tools (formatter / LSP / VS Code extension)
  live in a sibling repository.
- **ADR-0010** — parser core lives in `P4suta/aozora`. afm is the
  Markdown-dialect glue; the lex / borrowed-AST / renderer / gaiji
  table all live in the sibling repo.

ADR-0003..0008 are marked MOVED — their subject matter migrated to the
sibling `aozora` repo. Read them there if you need the rationale.

## Sibling projects

The 青空文庫 parser core (`aozora-syntax` / `aozora-pipeline` / `aozora-render`
/ `aozora-encoding` / `aozora-spec` / `aozora-proptest`) lives in
**[`P4suta/aozora`](https://github.com/P4suta/aozora)**. New 記法 work,
new lexer phases, new HTML class contracts, new renderer logic — all of
those land there, not here.

Authoring tools — `aozora-fmt` formatter, `aozora-lsp` Language Server,
VS Code extension, future editor plugins — live in
**[`P4suta/aozora-tools`](https://github.com/P4suta/aozora-tools)**.

afm itself stays focused on Markdown ↔ Aozora composition: how to
weave 青空文庫記法 into a CommonMark + GFM document so the HTML output
combines both correctly. New work in afm should fit that frame; if a
proposed change is "really an aozora parser change", route it to the
sibling repo first.

## Development environment

Docker is the only accepted execution surface. Host toolchain
invocations are forbidden in automation.

```
just build                # cargo build --workspace --all-targets
just test                 # cargo nextest run --workspace
just lint                 # fmt-check + clippy pedantic+nursery + typos + strict-code
just spec-commonmark      # CommonMark 0.31.2 spec (652 cases)
just spec-gfm             # GFM spec
just upstream-diff        # verify the upstream comrak tree is verbatim
just book-serve           # mdbook preview on http://localhost:3000
just ci                   # replicate the full CI pipeline locally

just watch [JOB]          # bacon watcher
just hooks                # install lefthook git hooks
just hooks-uninstall      # remove them
just sccache-stats        # sccache hit/miss ratio + cache size
```

## Host vs container tools

Host modern CLIs (`rg`, `fd`, `sd`, `jq`, `yq`, `bat`, `eza`, `just`, `jj`,
`atuin`, `starship`, `chezmoi`) are for exploration. Builds and test
runs go through `just` (Docker). The host/container split is enforced
by ADR-0002.

## Version control

Repo is colocated jj + git: jj manages the working copy and reflects
each operation into `.git/` so GitHub Actions and `git log` keep
working. Prefer `jj describe -m` + `jj bookmark move main --to @` +
`jj new`. Fall back to `git commit` only when scripting needs it.

## Where afm-side work tends to land

afm-markdown is a thin glue layer on top of two black boxes (vanilla
comrak + the aozora parser). Most new 青空文庫 features should land in
[`P4suta/aozora`](https://github.com/P4suta/aozora) — see that repo's
own CLAUDE.md. afm-side work falls into one of:

1. **CSS class contract drift** — `aozora-render` adds a new class.
   Update `crates/afm-markdown/src/test_support.rs::AFM_CLASSES` and
   the corresponding rule in `crates/afm-book/theme/afm-{horizontal,
   vertical}.css`.
2. **HTML post-process edge case** — block-sentinel paragraph parsing,
   inline sentinel substitution. Live in
   `crates/afm-markdown/src/post_process.rs`. Add a unit test in the
   same module's `#[cfg(test)] mod tests`.
3. **Public API drift** — `Options::afm_default` defaults, new entry
   points, diagnostic shape. Lives in `crates/afm-markdown/src/lib.rs`.
4. **CLI behaviour** — `crates/afm-cli/src/main.rs` and
   `crates/afm-cli/tests/cli_integration.rs`.
5. **Spec / golden / corpus regression** — `crates/afm-markdown/tests/*.rs`.

## Where to find what

```text
crates/afm-markdown/src/
  lib.rs           # Options, Rendered, render_to_string, serialize
  html.rs          # html::render_to_string(input) shim
  post_process.rs  # splice_aozora_html (HTML sentinel substitution)
  test_support.rs  # AFM_CLASSES + HTML invariant helpers (no AST)

crates/afm-markdown/tests/
  *.rs             # integration tests

crates/afm-cli/
  src/main.rs                # afm render / afm check, --encoding, --strict
  tests/cli_integration.rs   # binary-level integration

crates/afm-book/
  theme/afm-horizontal.css   # left-to-right writing-mode theme
  theme/afm-vertical.css     # vertical-rl (tategaki) theme

spec/
  aozora/fixtures/56656/     # 罪と罰 SJIS + UTF-8 + golden HTML
  aozora/cases/*.json        # hand-written annotation cases
  commonmark-0.31.2.json     # vendored spec
  gfm-0.29-gfm.json          # vendored spec

docs/
  adr/                       # 0001 … 0010
  specs/aozora/              # vendored Aozora Bunko annotation spec

upstream/comrak/             # v0.52.0 verbatim — 0-line diff (ADR-0001)
  COMRAK_SHA                 # pinned upstream sha
  UPSTREAM_DIFF.md           # diff-budget policy
```

## DO NOT

- **Do not modify `upstream/comrak/`.** The diff budget is 0. If you
  genuinely need a comrak change, that is a fork divergence decision
  and requires its own ADR.
- **Do not re-introduce parse-time or render-time hooks in comrak.**
  ADR-0001 and ADR-0008 together demand that `upstream/comrak/`
  carries no Aozora-aware code.
- **Do not bypass `afm-markdown`** when adding a new feature. afm-cli
  and downstream consumers should consume the public API
  (`render_to_string` / `serialize` / `Options`), not poke at
  `aozora-syntax` / `aozora-pipeline` / `aozora-render` directly. That
  keeps the surface tested in one place.
- **Do not put 青空文庫 parser logic here.** New 記法, lexer phase
  changes, AST shape changes — all of those go into
  [`P4suta/aozora`](https://github.com/P4suta/aozora). afm tracks the
  result via the `[workspace.dependencies]` configuration.
- **Do not bypass the Tier-A canary.** No bare `［＃` may appear in the
  HTML outside the `afm-annotation` hidden wrapper. The lib-internal
  tests in `crates/afm-markdown/src/lib.rs` enforce this.
- **Do not run cargo / mdbook / node directly on the host.** `just` +
  Docker is the only sanctioned path (ADR-0002).
- **Do not suppress warnings** (`#[allow(...)]`, `continue-on-error`,
  etc.) without a matching `reason = "..."` and a strict-code
  exemption. `just strict-code` will reject most cases; if you must
  add an exemption, document it in the surrounding code.
- **Do not pin dependency versions from memory.** Verify against
  crates.io / GitHub Releases at decision time. Especially for the
  `aozora` dependency — bumping it requires walking the borrowed-AST
  surface for breaking changes.

## Where Aozora-only fixtures live now

Aozora-layer test surface — `spec-aozora` (hand-written annotation
cases), `spec-golden-56656` (罪と罰 acceptance gate), and
`corpus-sweep` (17 k-work I1–I4 invariant sweep) — lives in the
sibling [`P4suta/aozora`](https://github.com/P4suta/aozora) repo. Run
them from there. The afm side keeps only the CommonMark+GFM spec
runners (`spec-commonmark` / `spec-gfm`) and the Aozora × Markdown
integration tests in `crates/afm-markdown/tests/`.
