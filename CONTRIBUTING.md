# Contributing to afm

Thanks for wanting to help. afm is an active project with a small
surface area of rules, but those rules are strict — the guarantees
below only stay true if every contribution respects them.

## Where things live

afm is the **Markdown ↔ Aozora composition layer**: it composes a
vendored verbatim comrak with the sibling
[`P4suta/aozora`](https://github.com/P4suta/aozora) parser /
renderer to produce HTML where CommonMark + GFM and 青空文庫記法
coexist correctly. New 青空文庫記法 / lexer / per-node render work
lands in the sibling repo, not here. afm-side work falls into:

1. **Markdown ↔ Aozora glue** — `crates/afm-markdown/`
   (`render_to_string`, `render_to_ir`, `serialize`,
   `post_process::splice_aozora_html`, the `IrDocument` projection).
2. **CLI** — `crates/afm-cli/` (`afm render` / `afm check`).
3. **WASM bridge** — `crates/afm-wasm/` (afm-obsidian / browser
   hosts).
4. **Documentation** — `crates/afm-book/` (mdbook site) and
   `docs/adr/` (Architecture Decision Records).

Authoring tools (formatter / LSP / VS Code extension) live in a
third sibling repo
[`P4suta/aozora-tools`](https://github.com/P4suta/aozora-tools)
per ADR-0009.

## Ground rules

1. **Docker-only execution** (ADR-0002). Do not invoke `cargo`,
   `mdbook`, or `playwright` on the host. Every automated step goes
   through `just <target>`, which shells into the dev container.
2. **Vendored comrak is hands-off** (ADR-0001). The fork sits at
   `upstream/comrak/` with a **0-line diff budget** — any change
   would be a fork divergence and needs its own ADR. Composition
   happens in `afm-markdown::post_process` (HTML sentinel splice),
   never inside `upstream/comrak/`.
3. **No warning suppressions.** `#[allow(...)]`, `#![allow(...)]`,
   `#[cfg_attr(..., allow(...))]`, `continue-on-error` in
   workflows, and similar escape hatches are rejected by
   `just strict-code`. Refactor the real issue instead.
4. **TDD with C1 100% branch coverage as the goal.** A failing
   test lands first, then the fix. The CI floor is currently 96%
   regions (`_COV_FLOOR` in `Justfile`), ratcheted upward as gaps
   close.

## First-time setup

```sh
docker compose build dev       # ~5 min first time, cached afterward
jj git init --colocate         # if jj isn't already initialised (optional)
just hooks                     # wire lefthook pre-commit / commit-msg / pre-push
just test                      # confirm green
```

## Development loop

```sh
just watch                     # bacon watcher inside the dev container
just lint                      # fmt + clippy pedantic+nursery + typos + strict-code
just test                      # full workspace nextest
just prop                      # property-based sweep (128 cases per block)
just spec-commonmark           # CommonMark 0.31.2 (652 cases)
just spec-gfm                  # GFM 0.29 spec
just coverage                  # cargo llvm-cov, fails below _COV_FLOOR
just upstream-diff             # verify upstream/comrak/ is still 0-line
just book-build                # mdbook build into crates/afm-book/book
just ci                        # replica of the full CI pipeline

# Before a release:
just prop-deep                 # 4096 cases per block — deeper than CI
```

`just --list` enumerates everything available.

Aozora-layer fixtures (annotation cases, golden 56656, 17 k-work
corpus sweep, fuzz) live in the sibling
[`P4suta/aozora`](https://github.com/P4suta/aozora) repo. Run them
from there.

## How to make a change

### afm-markdown (the glue layer)

Most afm-side changes are one of:

- **HTML post-process edge case** — landing in
  `crates/afm-markdown/src/post_process.rs`. Add a unit test in the
  same module's `#[cfg(test)] mod tests` and a property-test ping
  in `tests/post_process_invariants.rs` if the change has
  cross-input semantics. Predicates that codify "must-never-be"
  HTML shapes live in `crates/afm-markdown/src/test_support.rs` —
  add new ones there with both the predicate and a unit pin
  (`invariant_unit_check_X_passes_on_clean_input`,
  `invariant_unit_check_X_fires_on_<shape>`).
- **IR projection** — `crates/afm-markdown/src/ir.rs`. Each
  Aozora node maps via `project_inline` / `project_block_leaf`;
  paragraph dispatch flows through `IrWalker::walk_top` →
  `classify_paragraph`. Streaming-mode behaviour goes through
  `StreamingIrBuilder`. Add tests in
  `crates/afm-markdown/tests/ir_aozora.rs`.
- **CSS class contract drift** — when the sibling renderer adds a
  new `aozora-*` class, update
  `crates/afm-markdown/src/test_support.rs::AFM_CLASSES` (the brand
  is rewritten to `afm-*` per ADR-0011) and the corresponding rule
  in `crates/afm-book/theme/afm-{horizontal,vertical}.css`.
- **Public API drift** — `Options::afm_default` defaults, new
  entry points, diagnostic shape. Lives in
  `crates/afm-markdown/src/lib.rs`. Bumping the IR schema is a
  semver-major change because afm-wasm and afm-obsidian validate
  it on the JS side.
- **CLI behaviour** — `crates/afm-cli/src/main.rs` and the binary-
  level integration tests in `crates/afm-cli/tests/cli_integration.rs`.

### Spec / golden / corpus regression

`crates/afm-markdown/tests/*.rs` covers the CommonMark + GFM spec
runners (`commonmark_spec.rs`, `gfm_spec.rs`) and the Aozora ×
Markdown integration surface (`aozora_parity.rs`,
`paired_container.rs`, `heading_promotion.rs`,
`block_structure_interaction.rs`,
`property_html_shape.rs`, …). Each adds a single layer of evidence;
new invariants ride on the same scaffolding.

### Adding a 青空文庫 notation

This **does not happen on the afm side**. Lexer phases, AST
shapes, recogniser tables, and per-node renderers all live in the
sibling [`P4suta/aozora`](https://github.com/P4suta/aozora) repo
(see ADR-0010 for the rationale). Once a new construct is
classified upstream and lands in `aozora-syntax`, afm picks it up
automatically through the workspace dep — usually with a one-line
mapping in `afm_markdown::ir::project_inline` /
`project_block_leaf` plus a test, and a `AFM_CLASSES` update if
the renderer adds a new CSS hook.

## Architectural changes

Any decision that shapes how a whole subsystem behaves lands first
as an **Architecture Decision Record** (MADR format) under
`docs/adr/`. Scaffold one with:

```sh
cargo xtask new-adr 'my new decision'
```

Reference the ADR in the commit body. Look at
`docs/adr/0008-MOVED.md` (and the canonical text in the sibling
repo) for an example of a decision that fundamentally reshaped the
pipeline; look at `docs/adr/0011-brand-boundary-css-class-rewrite.md`
for an example of a small, scoped boundary decision.

## Commit style

**Conventional Commits**
([v1.0.0](https://www.conventionalcommits.org/)). The `commit-msg`
hook enforces this. Accepted types: `feat`, `fix`, `docs`,
`style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`,
`revert`. Scopes match the workspace shape — one of: `markdown`
(afm-markdown), `cli` (afm-cli), `wasm` (afm-wasm), `book`
(afm-book), `xtask`, `comrak` (touches under `upstream/comrak/`),
`adr`, `release`, `dev`, `test`.

A single commit should be a single logical change. Split unrelated
edits.

## Pull requests

- PR title should be `<type>(<scope>): <summary>` matching the
  commits.
- Link any issue the PR closes (`Closes #N` in the body).
- The PR template (`.github/PULL_REQUEST_TEMPLATE.md`) walks you
  through the checklist — **keep it**. It reminds everyone
  (including the author) of the full gate: tests, coverage, ADR,
  `just ci`.
- CI runs `just ci` in the ci container image. The gate is the
  same one you ran locally; surprises mean either an environment
  mismatch or an ADR-boundary subtlety.

## Reporting bugs and asking for features

- **Bugs**: use the `bug_report` issue form. The minimal
  reproducible input (the shortest source text that triggers the
  issue) is the most valuable thing you can supply.
- **Features**: use the `feature_request` form. Concrete
  motivation (a real Aozora Bunko text that needs the notation, a
  CommonMark construction that would benefit, a corpus sweep hit)
  makes triage faster. If the feature is "support a new 青空文庫
  notation", file it on the sibling
  [`P4suta/aozora`](https://github.com/P4suta/aozora/issues) repo
  instead — that's where the parser lives.
- **Questions / discussions**: prefer GitHub Discussions over
  issues.

## Security

Security-sensitive issues (parser crashes, memory safety concerns,
sandbox escapes) should be reported privately per `SECURITY.md` —
do **not** open a public issue.

## How to release

Releases are triggered by a git tag of the form `v<semver>`:

1. Update `CHANGELOG.md` — promote `[Unreleased]` to
   `[<version>] - YYYY-MM-DD` and add a fresh `[Unreleased]` stub.
2. Commit the changelog bump:
   `git commit -m "chore: release v<version>"`.
3. Tag (annotated): `git tag -a v<version> -m 'v<version>'`.
4. Push: `git push origin main v<version>`.
5. `.github/workflows/release.yml` reacts to the tag, builds
   release binaries on five targets (linux-gnu, linux-musl,
   macos-aarch64, macos-x86_64, windows-msvc), assembles tarballs
   with the `afm` binary, `LICENSE-MIT`, `LICENSE-APACHE`,
   `NOTICE`, and `README.md`, and uploads the archives plus
   `SHA256SUMS` to the GitHub Release.
6. Sanity check: download one artefact, run `sha256sum --check`,
   then `./afm --version` to confirm the embedded version matches
   the tag.

A dry-run is available via `workflow_dispatch` from the
[Actions tab](https://github.com/P4suta/afm/actions/workflows/release.yml) —
trigger it from `main` or a release branch before cutting the tag
to confirm the five-target matrix builds cleanly.

**ADR-0002 scope exception**: release builds run on native GitHub
Actions runners with the matching stable rustc, not inside the dev
Docker image. The Docker-only rule applies to development and CI;
the release pipeline is deliberately host-toolchain so each binary
target matches its runner OS exactly. See the leading comment in
`release.yml` for the full rationale.

## License

By contributing, you agree that your contributions are
dual-licensed under Apache-2.0 OR MIT, the same as the project.
