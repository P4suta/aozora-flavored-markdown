# Aozora Flavored Markdown (afm)

[English](./README.md) · [日本語](./README.ja.md)

<p align="center">
  <a href="https://github.com/P4suta/afm/actions/workflows/ci.yml"><img alt="ci" src="https://github.com/P4suta/afm/actions/workflows/ci.yml/badge.svg"></a>
  <a href="https://github.com/P4suta/afm/actions/workflows/docs.yml"><img alt="docs deploy" src="https://github.com/P4suta/afm/actions/workflows/docs.yml/badge.svg"></a>
  <a href="https://github.com/P4suta/afm/releases/latest"><img alt="latest release" src="https://img.shields.io/github/v/release/P4suta/afm?display_name=tag&sort=semver"></a>
  <a href="./LICENSE-APACHE"><img alt="license" src="https://img.shields.io/badge/license-Apache--2.0%20OR%20MIT-blue"></a>
  <a href="./rust-toolchain.toml"><img alt="msrv" src="https://img.shields.io/badge/rust-1.95%2B-orange"></a>
  <a href="https://codespaces.new/P4suta/afm"><img alt="open in github codespaces" src="https://github.com/codespaces/badge.svg" height="20"></a>
</p>

<p align="center">
  📖 <a href="https://p4suta.github.io/afm/"><strong>Documentation site</strong></a>
  · 🧪 <a href="https://p4suta.github.io/afm/api/"><strong>API reference (rustdoc)</strong></a>
  · 📦 <a href="https://github.com/P4suta/afm/releases"><strong>Releases &amp; binaries</strong></a>
  · 📝 <a href="./CHANGELOG.md"><strong>Changelog</strong></a>
</p>

**Aozora Flavored Markdown** (afm) is a Markdown dialect, modelled after
[GitHub Flavored Markdown (GFM)](https://github.github.com/gfm/), that
layers Aozora Bunko (青空文庫) typography — ruby, bouten, 縦中横,
`［＃…］` annotations, gaiji, accent decomposition — on top of
CommonMark + GFM for Japanese vertical and horizontal writing.

Like GFM, afm is a **strict superset** of CommonMark + GFM: any pure
CommonMark / GFM document parses identically under afm, and the Aozora
extensions kick in only where the input actually uses them. The file
extension remains `.md`. A single Rust crate set and a single `afm`
binary drop into the same slot you would otherwise use a CommonMark
parser in.

This repository hosts both the **specification** of afm (rendered as
the [mdbook site](https://p4suta.github.io/afm/) under
[`crates/afm-book/`](./crates/afm-book/)) and its **reference
implementation** — the same split GFM uses.

## The lineage

Each dialect in the Markdown family extends the one before it. afm is the
Japanese-typography layer:

```
CommonMark  ──▶  GFM  ──▶  Aozora Flavored Markdown
(structural       (tables,     (ruby, bouten, 縦中横, 字下げ, 外字,
 Markdown)         task lists,  返り点, 割注, アクセント分解, …)
                   ~strikethrough~)
```

The Aozora Bunko community has maintained
[a rich annotation notation](https://www.aozora.gr.jp/annotation/) for
typesetting Japanese prose for over twenty years. afm picks it up
wholesale, maps it onto a modern Markdown AST, and lets you embed the
result in any pipeline that speaks CommonMark.

## Hard guarantees

- **100% CommonMark / GFM compatibility** — the full spec test suites
  pass verbatim (652 CommonMark 0.31.2 cases + the GFM 0.29 cases).
- **100% Aozora Bunko compatibility target** — every notation listed at
  <https://www.aozora.gr.jp/annotation/> parses, and the flagship
  『罪と罰』 fixture (Aozora Bunko card 56656) holds a Tier-A acceptance
  gate (panic-free, no unconsumed `［＃` markers in the rendered HTML).
- **17 k-work corpus sweep** — four invariants gated in CI: I1 no panic,
  I2 no bare `［＃` leak, I3 `serialize ∘ parse` fixed point, I4 HTML
  tag-balanced. See ADR-0007.
- **Single binary**, no runtime process dependencies.
- **Pure-functional parse pipeline** (ADR-0008) — zero parse-time hooks
  in vendored comrak; Aozora recognition lives in
  [`aozora`](https://github.com/P4suta/aozora) (sibling repo) and is
  spliced into the comrak AST by `afm-markdown::post_process`.

## What you can write

```markdown
# 第一章                             (Markdown heading)
［＃「第一篇」は大見出し］            (Aozora heading, aliased to the same AST)

彼は｜青梅《おうめ》に行った。        (Ruby)
それは《《強調したい》》ことだった。    (Bouten / emphasis dots)
令和［＃縦中横］2［＃縦中横終わり］年。 (Tate-chu-yoko)

［＃ここから字下げ］                  (Block indent)
段落……
［＃ここで字下げ終わり］
```

## Workspace layout

```
afm/
  upstream/comrak/           # vendored comrak 0.52.0, verbatim (0-line diff since v0.2.4)
  crates/
    afm-markdown/            # CommonMark + GFM + 青空文庫記法 HTML integration layer
    afm-cli/                 # `afm` binary (render / check)
    afm-book/                # mdbook documentation site (excluded from cargo workspace)
    xtask/                   # upstream-sync, spec-refresh, new-adr
  spec/                      # CommonMark / GFM / Aozora fixtures
  docs/adr/                  # Architecture Decision Records
```

The Aozora-specific lexer / AST / renderer (`aozora-syntax`,
`aozora-lexer`, `aozora-parser`, `aozora-encoding`, `aozora-corpus`,
`aozora-test-utils`) live in the sibling
[`P4suta/aozora`](https://github.com/P4suta/aozora) repository and are
consumed here as a tag-pinned `git` dependency (ADR-0010).

## Sibling repositories

| Repo | What it is |
|---|---|
| [`P4suta/aozora`](https://github.com/P4suta/aozora) | Pure 青空文庫記法 parser — lexer, AST, renderer, gaiji table. |
| [`P4suta/aozora-tools`](https://github.com/P4suta/aozora-tools) | Authoring tools: `aozora-fmt` formatter, `aozora-lsp` Language Server, tree-sitter grammar, VS Code extension. |

## Development

All operations run inside Docker. The host toolchain is never invoked
directly (ADR-0002).

```bash
just test              # cargo nextest via Docker
just lint              # fmt + clippy + typos + strict-code
just coverage          # llvm-cov regions, CI floor at 96%
just spec-commonmark   # full CommonMark 0.31.2 spec
just spec-gfm          # GFM 0.29 spec
just spec-aozora       # hand-written Aozora fixtures
just spec-golden-56656 # 罪と罰 Tier-A acceptance gate
just corpus-sweep      # 17 k-work invariant sweep (I1–I4)
just upstream-diff     # verify the upstream comrak tree stays 0-line (verbatim v0.52.0)
just ci                # replicate the full CI matrix locally
just book-serve        # mdbook live preview at http://localhost:3000
```

See [CLAUDE.md](./CLAUDE.md) for the project guide, [docs/adr/](./docs/adr/)
for architectural decisions, and [CONTRIBUTING.md](./CONTRIBUTING.md) for
how to hack on afm.

## Examples

Short end-to-end snippets live under
[`crates/afm-markdown/examples/`](./crates/afm-markdown/examples/):

- `render-utf8.rs` — parse a UTF-8 file and emit HTML on stdout.
- `render-sjis.rs` — parse a Shift_JIS Aozora Bunko text via `aozora-encoding`.
- `ast-walk.rs` — walk the parsed AST and tally AozoraNode variants.
- `serialize-round-trip.rs` — verify `serialize ∘ parse ≡ id` on one file.

Run any of them with:

```sh
cargo run --example <name> -p afm-markdown -- <path/to/input.md>
```

## Install

Pre-built binaries for **Linux x86_64**, **macOS arm64**, and **Windows
x86_64** are attached to every GitHub Release — see
[the releases page](https://github.com/P4suta/afm/releases) and pick a
`afm-vX.Y.Z-<target>.{tar.gz,zip}`. SHA256 sums are published as
`SHA256SUMS` next to the archives.

Or build from source:

```sh
cargo install --git https://github.com/P4suta/afm --tag v0.2.3 --locked afm-cli
```

## Security

Vulnerabilities go through GitHub Security Advisories — see
[`SECURITY.md`](./SECURITY.md) for the disclosure flow.

## License

Dual-licensed under [Apache-2.0](./LICENSE-APACHE) OR [MIT](./LICENSE-MIT)
at your option, matching Rust community convention.

The vendored `upstream/comrak/` tree remains under its upstream license
(BSD-2-Clause). See `upstream/comrak/COPYING`.

Sample texts under `spec/aozora/fixtures/` are sourced from 青空文庫
(public domain) and attributed per work.

See [NOTICE](./NOTICE) for the full third-party attribution index
(vendored comrak, CommonMark / GFM spec fixtures, Aozora Bunko material).
