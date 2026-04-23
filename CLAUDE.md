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
- 100 % CommonMark 0.31.2 + GFM compatibility (verbatim spec tests pass).
- 100 % Aozora Bunko compatibility (every notation at
  <https://www.aozora.gr.jp/annotation/> parses; the 『罪と罰』 fixture is the
  day-1 golden).
- Single binary, no runtime process dependencies.
- TDD with C1 branch coverage 100 % on `aozora/**`, `afm-syntax`, `afm-encoding`.

## Architecture map

Five crates in `crates/`, one vendored fork under `upstream/`:

| Crate | Responsibility | Depends on |
|---|---|---|
| `afm-syntax` | `AozoraNode` AST + `AozoraExtension` trait + accent table | nothing internal |
| `afm-parser` | `AfmAdapter` impl, preparse rewrite, Aozora recognisers, HTML renderer | `afm-syntax`, `comrak` |
| `afm-encoding` | Shift_JIS decoding, UTF-8 BOM sniff, gaiji resolution (M2) | `afm-syntax` |
| `afm-cli` | `afm` binary (render / check subcommands) | `afm-parser`, `afm-encoding` |
| `afm-book` | mdbook documentation site | — |
| `xtask` | dev automation (upstream-diff, spec-refresh, new-adr, corpus-*) | — |
| `upstream/comrak/` | vendored fork v0.52.0 — **200-line diff budget** (see ADR-0001) | — |

## Architecture Decision Records

Read these before touching anything the ADR governs. `docs/adr/` on disk.

- **ADR-0001** — fork comrak and vendor in-tree; 200-line diff budget enforced.
- **ADR-0002** — Docker-only execution: every tool runs via `docker compose run` through `Justfile` targets, never directly on the host.
- **ADR-0003** — afm-parser architecture: `Arc<dyn AozoraExtension + 'c>` trait object on comrak's `Extension.aozora`; single `NodeValue::Aozora(AozoraNode)` variant.
- **ADR-0004** — accent decomposition preparse: rewrite 〔…〕 bodies to Unicode before comrak sees them (prevents lone `` ` `` from opening a CommonMark code span).
- **ADR-0005** (M1) — paired block annotation container hook. TBD.

## Development environment

Docker is the only accepted execution surface. Host toolchain invocations
(`cargo test`, `mdbook build`, `playwright`, …) are forbidden in automation.
`just` runs on the host and shells through `docker compose run`.

```
just build                # cargo build --workspace --all-targets
just test                 # cargo nextest run --workspace
just lint                 # fmt-check + clippy pedantic+nursery + typos
just coverage             # llvm-cov branch, 100 % gate on our code
just spec-commonmark      # CommonMark 0.31.2 spec (652 cases)
just spec-gfm             # GFM spec
just spec-aozora          # hand-written aozora fixtures
just spec-golden-56656    # 『罪と罰』 Tier A — M0 acceptance gate
just upstream-diff        # enforce 200-line budget vs comrak v0.52.0
just book-serve           # mdbook preview on http://localhost:3000
just ci                   # replicate the full CI pipeline locally
just adr '<title>'        # scaffold a new ADR via xtask
just upstream-sync <tag>  # sync vendored comrak to a new tag
```

Bootstrap steps for a fresh clone:

```
docker compose build dev      # ~5 min first time, cached after
jj git init --colocate        # if jj isn't already initialised
docker compose run --rm dev lefthook install   # optional
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
so GitHub Actions and `git log` both keep working. Prefer `jj commit -m` for
new work; fall back to `git commit` only when scripting or when a tool
requires it.

## TDD flow for new features

A feature lands in this order (per ADR-0003 §Staged implementation):

1. **Spec fixture** — add a `spec/aozora/<kind>.json` case with the input +
   expected AST or HTML.
2. **AST variant** — if a new `AozoraNode` variant is needed, add it to
   `crates/afm-syntax/src/lib.rs` with `#[non_exhaustive]`.
3. **Parser test (red)** — write the failing integration test that exercises
   the feature.
4. **Parser implementation** — extend the recognisers under
   `crates/afm-parser/src/aozora/*`. Dispatch lives in `adapter.rs`.
5. **Renderer test (red)** — assert the expected HTML shape.
6. **Renderer implementation** — add the branch to
   `crates/afm-parser/src/aozora/html.rs`.
7. **Snapshot** — `cargo insta review` to accept any snapshot drift.
8. **Verify** — `just lint && just test && just spec-golden-56656`.

No commit lands without the full gate passing locally.

## Where to find what

```
crates/afm-syntax/src/
  lib.rs              # AozoraNode enum, classifier methods
  extension.rs        # AozoraExtension trait + contexts
  accent.rs           # 114-entry accent decomposition table

crates/afm-parser/src/
  lib.rs              # public parse() + Options
  adapter.rs          # AfmAdapter impl — inline + block dispatch
  preparse.rs         # accent decomposition inside 〔...〕
  html.rs             # comrak-based render_to_string wrapper
  test_support.rs     # test helpers (cfg(test))
  aozora/
    ruby.rs           # explicit + implicit ruby parser
    inline.rs         # (M1) inline recognisers (bouten 《《》》, etc.)
    block.rs          # (M1) block annotation dispatch
    bouten.rs         # (M1) bouten forward-reference parser
    tcy.rs            # (M1) 縦中横
    annotation.rs     # (M1) ［＃…］ keyword dispatcher
    html.rs           # per-variant HTML emitter

crates/afm-parser/tests/
  golden_56656.rs     # 罪と罰 Tier A acceptance
  commonmark_spec.rs  # (M1) CommonMark 0.31.2 (652 cases)
  gfm_spec.rs         # (M1) GFM spec
  aozora_spec.rs      # (M1) hand-written fixtures runner
  forward_reference_regression.rs
  long_paragraph_regression.rs
  bisect_regression.rs  # #[ignore] diagnostic harness

spec/
  aozora/fixtures/56656/ # 罪と罰 SJIS + UTF-8 + golden HTML
  commonmark-0.31.2.json # (M1 B3) vendored spec
  gfm-0.29-gfm.json      # (M1 B3) vendored spec
  aozora/*.json          # (M1) hand-written annotation cases

docs/
  adr/                 # ADR-0001 … 0005+
  specs/aozora/        # vendored Aozora Bunko annotation spec pages
  plan.md              # milestone plan snapshot

upstream/comrak/      # v0.52.0 verbatim + fixed hook points
  COMRAK_SHA          # pinned upstream sha
  UPSTREAM_DIFF.md    # diff-budget policy
```

## DO NOT

- **Do not modify `upstream/comrak/` without an ADR.** The 200-line diff
  budget and quarterly sync strategy depend on every change being an explicit
  hook addition, not ad-hoc logic.
- **Do not run cargo / mdbook / node directly on the host.** `just` +
  Docker is the only sanctioned path (ADR-0002).
- **Do not suppress warnings** (`#[allow]`, `continue-on-error`, etc.).
  Memory note `feedback_no_warning_suppression` — fix root causes.
- **Do not guess at Aozora encoding edge cases.** Read
  `docs/specs/aozora/*.html` or the live page before patching. See memory note
  `feedback_read_aozora_spec_first`.
- **Do not pin dependency versions from memory.** Verify against
  crates.io / npm / GitHub Releases at decision time.

## Current status (2026-04-23)

- M0 Spike complete: 13 commits, `e69ddd8 … e853e00` on `main`.
- Tier A acceptance on 『罪と罰』 passes.
- Workspace tests 95/95, comrak tests 614/614, clippy pedantic+nursery clean.
- upstream diff: 169/200 lines.
- M1 Core 記法 underway per `docs/plan.md` (approved 2026-04-23).
