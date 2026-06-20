# Aozora Flavored Markdown

[English](./README.md) · [日本語](./README.ja.md)

<p align="center">
  <a href="https://github.com/P4suta/aozora-flavored-markdown/actions/workflows/ci.yml"><img alt="ci" src="https://github.com/P4suta/aozora-flavored-markdown/actions/workflows/ci.yml/badge.svg"></a>
  <a href="https://github.com/P4suta/aozora-flavored-markdown/actions/workflows/docs.yml"><img alt="docs deploy" src="https://github.com/P4suta/aozora-flavored-markdown/actions/workflows/docs.yml/badge.svg"></a>
  <a href="https://crates.io/crates/aozora-flavored-markdown-cli"><img alt="crates.io" src="https://img.shields.io/crates/v/aozora-flavored-markdown-cli?label=aozora-flavored-markdown-cli"></a>
  <a href="https://docs.rs/aozora-flavored-markdown"><img alt="docs.rs" src="https://img.shields.io/docsrs/aozora-flavored-markdown?label=docs.rs"></a>
  <a href="https://github.com/P4suta/aozora-flavored-markdown/releases/latest"><img alt="latest release" src="https://img.shields.io/github/v/release/P4suta/aozora-flavored-markdown?display_name=tag&sort=semver"></a>
  <a href="./LICENSE-APACHE"><img alt="license" src="https://img.shields.io/badge/license-Apache--2.0%20OR%20MIT-blue"></a>
  <a href="./rust-toolchain.toml"><img alt="msrv" src="https://img.shields.io/badge/rust-1.96%2B-orange"></a>
  <a href="https://codespaces.new/P4suta/aozora-flavored-markdown"><img alt="open in github codespaces" src="https://github.com/codespaces/badge.svg" height="20"></a>
</p>

<p align="center">
  📖 <a href="https://p4suta.github.io/aozora-flavored-markdown/"><strong>Documentation site</strong></a>
  · 🧪 <a href="https://p4suta.github.io/aozora-flavored-markdown/api/"><strong>API reference (rustdoc)</strong></a>
  · 📦 <a href="https://github.com/P4suta/aozora-flavored-markdown/releases"><strong>Releases &amp; binaries</strong></a>
  · 📝 <a href="./CHANGELOG.md"><strong>Changelog</strong></a>
</p>

**Aozora Flavored Markdown** is a Markdown dialect, modelled after
[GitHub Flavored Markdown (GFM)](https://github.github.com/gfm/), that
layers Aozora Bunko (青空文庫) typography — ruby, bouten, 縦中横,
`［＃…］` annotations, gaiji, accent decomposition — on top of
CommonMark + GFM for Japanese vertical and horizontal writing.

Like GFM, aozora-flavored-markdown is a **strict superset** of CommonMark + GFM: any pure
CommonMark / GFM document parses identically under aozora-flavored-markdown, and the Aozora
extensions kick in only where the input actually uses them. The file
extension remains `.md`. A single Rust crate set and a single `aozora-flavored-markdown`
binary drop into the same slot you would otherwise use a CommonMark
parser in.

This repository hosts both the **specification** of aozora-flavored-markdown (rendered as
the [mdbook site](https://p4suta.github.io/aozora-flavored-markdown/) under
[`crates/aozora-flavored-markdown-book/`](./crates/aozora-flavored-markdown-book/)) and its **reference
implementation** — the same split GFM uses.

## The lineage

Each dialect in the Markdown family extends the one before it. aozora-flavored-markdown is the
Japanese-typography layer:

```
CommonMark  ──▶  GFM  ──▶  Aozora Flavored Markdown
(structural       (tables,     (ruby, bouten, 縦中横, 字下げ, 外字,
 Markdown)         task lists,  返り点, 割注, アクセント分解, …)
                   ~strikethrough~)
```

The Aozora Bunko community has maintained
[a rich annotation notation](https://www.aozora.gr.jp/annotation/) for
typesetting Japanese prose for over twenty years. aozora-flavored-markdown picks it up
wholesale, maps it onto a modern Markdown AST, and lets you embed the
result in any pipeline that speaks CommonMark.

## Hard guarantees

- **100% CommonMark / GFM compatibility** — the full spec test suites
  pass verbatim (652 CommonMark 0.31.2 cases + the GFM 0.29 cases).
- **100% Aozora Bunko compatibility target** — every notation listed at
  <https://www.aozora.gr.jp/annotation/> parses; aozora-flavored-markdown preserves the
  Tier-A invariant (no unconsumed `［＃` markers in the rendered HTML).
- **Single binary**, no runtime process dependencies.
- **Pure-functional parse pipeline** — zero parse-time hooks in
  vendored comrak; Aozora recognition lives in
  [`aozora`](https://github.com/P4suta/aozora) (sibling repo) and is
  spliced into the comrak AST by `aozora-flavored-markdown::ast_splice`.

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
aozora-flavored-markdown/
  upstream/comrak/           # vendored comrak 0.52.0, verbatim (0-line diff)
  crates/
    aozora-flavored-markdown/            # CommonMark + GFM + 青空文庫記法 HTML integration layer
    aozora-flavored-markdown-cli/                 # `aozora-flavored-markdown` binary (render / check)
    aozora-flavored-markdown-book/                # mdbook documentation site (excluded from cargo workspace)
    xtask/                   # upstream-sync, spec-refresh, new-adr
  spec/                      # CommonMark / GFM / Aozora fixtures
  docs/adr/                  # Architecture Decision Records
```

The Aozora-specific lexer / AST / renderer (`aozora-syntax`,
`aozora-pipeline`, `aozora-render`, `aozora-encoding`, `aozora-spec`,
`aozora-proptest`) live in the sibling
[`P4suta/aozora`](https://github.com/P4suta/aozora) repository and are
consumed here as a `git` dependency (ADR-0010).

## Sibling repositories

| Repo | What it is |
|---|---|
| [`P4suta/aozora`](https://github.com/P4suta/aozora) | Pure 青空文庫記法 parser — lexer, AST, renderer, gaiji table. |
| [`P4suta/aozora-tools`](https://github.com/P4suta/aozora-tools) | Authoring tools: `aozora-fmt` formatter, `aozora-lsp` Language Server, tree-sitter grammar, VS Code extension. |

## Development

All operations run inside Docker; the host toolchain is never invoked
directly (ADR-0002). After cloning, one command sets everything up:

```bash
just setup             # build the dev image, install hooks, check env, run tests
```

Then the inner loop — edit, see it compile, gate before pushing:

```bash
just watch             # bacon file-watcher: recompiles on save, inside Docker
just test              # cargo nextest via Docker
just lint              # fmt + clippy + typos + strict-code
just ci                # replicate the full CI gate set locally (run before pushing)
```

`just` with no arguments lists every recipe, grouped by area. Others you'll
reach for:

```bash
just coverage          # llvm-cov regions, CI floor at 96%
just spec-commonmark   # full CommonMark 0.31.2 spec
just spec-gfm          # GFM 0.29 spec
just upstream-diff     # verify the upstream comrak tree stays 0-line (verbatim v0.52.0)
just book-serve        # mdbook live preview at http://localhost:3000
```

Prefer a zero-install start? The **Open in GitHub Codespaces** badge at the top
boots a ready-to-code container with the toolchain already built.

Aozora-only test surfaces (`spec-aozora`, `spec-golden-56656`,
`corpus-sweep`) live in the sibling
[`P4suta/aozora`](https://github.com/P4suta/aozora) repo. Run them
from there.

See [CONTRIBUTING.md](./CONTRIBUTING.md) for how to hack on aozora-flavored-markdown and
[docs/adr/](./docs/adr/) for the architectural decisions behind it.

## Examples

Short end-to-end snippets live under
[`crates/aozora-flavored-markdown/examples/`](./crates/aozora-flavored-markdown/examples/):

- `render-utf8.rs` — parse a UTF-8 file and emit HTML on stdout.
- `render-sjis.rs` — parse a Shift_JIS Aozora Bunko text via `aozora-encoding`.
- `ast-walk.rs` — walk the parsed AST and tally AozoraNode variants.
- `serialize-round-trip.rs` — verify `serialize ∘ parse ≡ id` on one file.

Run any of them with:

```sh
cargo run --example <name> -p aozora-flavored-markdown -- <path/to/input.md>
```

## Install

### CLI

From crates.io:

```sh
cargo install aozora-flavored-markdown-cli
```

Pre-built binaries for **Linux x86_64**, **macOS arm64**, and **Windows
x86_64** are attached to every GitHub Release — see
[the releases page](https://github.com/P4suta/aozora-flavored-markdown/releases) and pick a
`aozora-flavored-markdown-vX.Y.Z-<target>.{tar.gz,zip}`. SHA256 sums are published as
`SHA256SUMS` next to the archives. Each archive bundles the binary, shell
completions, and the `aozora-flavored-markdown.1` man page.

Bleeding edge from git:

```sh
cargo install --git https://github.com/P4suta/aozora-flavored-markdown --locked aozora-flavored-markdown-cli
```

### Library

```sh
cargo add aozora-flavored-markdown
```

```rust
use aozora_flavored_markdown::{Options, render_to_string};

let rendered = render_to_string("彼は｜青梅《おうめ》に行った。", &Options::default());
assert!(rendered.html.contains("<ruby>"));
```

The full API is on [docs.rs](https://docs.rs/aozora-flavored-markdown). The
rendered HTML carries stable `aozora-md-*` CSS classes (see
[ADR-0011](docs/adr/0011-brand-boundary-css-class-rewrite.md)).

## Security

Vulnerabilities go through GitHub Security Advisories — see
[`SECURITY.md`](./SECURITY.md) for the disclosure flow.

## License

Dual-licensed under [Apache-2.0](./LICENSE-APACHE) OR [MIT](./LICENSE-MIT)
at your option, matching Rust community convention.

The vendored `upstream/comrak/` tree remains under its upstream license
(BSD-2-Clause). See `upstream/comrak/COPYING`.

Sample 青空文庫 texts used by the parser-side spec / golden / corpus
fixtures live in the sibling
[`P4suta/aozora`](https://github.com/P4suta/aozora) repo (public
domain, attributed per work). aozora-flavored-markdown itself ships only the CommonMark
0.31.2 and GFM 0.29 spec fixtures under `spec/`.

See [NOTICE](./NOTICE) for the full third-party attribution index
(vendored comrak, CommonMark / GFM spec fixtures, Aozora Bunko material).
