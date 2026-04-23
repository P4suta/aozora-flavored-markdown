# aozora-flavored-markdown (afm)

A Markdown dialect that merges CommonMark/GFM with Aozora Bunko (青空文庫) typography
for Japanese vertical and horizontal writing.

Positioned as a *dialect of Markdown* (not a new language). The file extension remains
`.md`, and pure CommonMark documents pass through afm without change.

## Hard guarantees

- **100% CommonMark / GFM compatibility** — the full spec test suites pass verbatim.
- **100% Aozora Bunko compatibility** — every notation listed at
  <https://www.aozora.gr.jp/annotation/> parses, and the regression CI renders a pinned
  120-work corpus (Tier A: panic-free + zero unconsumed `［＃`; Tier B: round-trip
  stable; Tier C: golden-HTML diff-clean from M3).

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
  upstream/comrak/           # vendored comrak 0.52.0, modified at fixed hook points only
  crates/
    afm-syntax/              # AozoraNode AST types (no parser dep)
    afm-parser/              # forked comrak + aozora extensions + html renderer
    afm-encoding/            # Shift_JIS + Aozora gaiji resolution
    afm-cli/                 # `afm` binary
    afm-book/                # mdbook documentation site
    xtask/                   # upstream-sync, upstream-diff, corpus-refresh, bench
  spec/                      # CommonMark/GFM fixtures + Aozora corpus lock
```

## Development

All operations run inside Docker. The host toolchain is never invoked directly.

```bash
just test              # cargo nextest via Docker
just lint              # fmt + clippy + typos
just coverage          # llvm-cov branch, CI gate at 100% on our code
just spec-commonmark   # full CommonMark spec pass
just corpus            # 120-work Aozora regression (Tier A + B)
just upstream-diff     # enforce 200-line budget against upstream comrak
just ci                # replicate the full CI matrix locally
just book-serve        # mdbook live preview
```

See `docs/adr/` for architectural decisions.

## Status

Pre-alpha. M0 Spike in progress. See [docs/plan.md](./docs/plan.md) for milestone
detail and [docs/adr/](./docs/adr/) for architectural decision records.

## License

Dual-licensed under [Apache-2.0](./LICENSE-APACHE) OR [MIT](./LICENSE-MIT) at your
option, matching Rust community convention.

The vendored `upstream/comrak/` tree remains under its upstream license
(BSD-2-Clause). See `upstream/comrak/COPYING`.

Sample texts under `spec/aozora/fixtures/` are sourced from 青空文庫 (public domain)
and attributed per work.
