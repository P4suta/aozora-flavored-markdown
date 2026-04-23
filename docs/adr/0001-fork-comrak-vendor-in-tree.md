# 0001. Fork comrak and vendor it in-tree

- Status: accepted
- Date: 2026-04-23
- Tags: architecture, parser, fork

## Context

afm must deliver **100% CommonMark + GFM** compatibility and also parse Japanese
typography constructs (ruby, bouten, tate-chu-yoko, `［＃...］` block annotations,
gaiji) that no upstream Markdown parser supports. The hard requirement is that an
annotated span like

```
彼は［＃「｜青梅《おうめ》に行った」に傍点］
```

must round-trip cleanly even when the annotation straddles links, lists, or emphasis.

Three Rust Markdown bases were evaluated:

- **comrak** — CommonMark+GFM-compliant, full AST, maintainer-active (v0.52.0 Apr 2026).
- **pulldown-cmark** — event-stream parser. Widely used.
- **markdown-rs** — re-implementation of wooorm's micromark. Strict about what it extends.

## Decision

Fork comrak at tag `v0.52.0` and vendor the tree at `/upstream/comrak/`. Extensions
live exclusively under `crates/afm-parser/src/aozora/` and touch upstream files only at
a handful of named hook points, under a hard **200-line diff budget** enforced by
`cargo xtask upstream-diff`.

## Consequences

Easier:
- CommonMark + GFM tests pass for free (vendored upstream ships them).
- Rich AST available from day one; we avoid re-implementing delimiter-run parsing.
- Hook points survive upstream refactors because the wrapped data type is a single
  `NodeValue::Aozora(AozoraNode)` variant.

Harder:
- Upstream drift: we must sync quarterly. Mitigation is the hook-minimal design —
  merges are three-way with at most a few conflicts.
- Crates.io publication requires the vendored tree under permissive licensing (comrak
  is BSD-2-Clause — compatible with our Apache-2.0 OR MIT).

## Alternatives considered

- **pulldown-cmark + event transform**: rejected because ruby parsing is intra-token
  (`｜漢字《かんじ》` must be detected inside text tokens), which the event stream
  exposes only *after* tokenisation. Reassembling reliably is effectively re-parsing.
- **markdown-rs fork**: rejected because its extension model accepts only standardised
  dialects (MDX, math, GFM). Our annotations are project-specific.
- **Fresh parser on `winnow`/`chumsky`**: rejected because the CommonMark spec is
  ~600 cases of edge behaviour we don't want to re-verify.

## References

- [Implementation plan](../../../.claude/plans/compiled-discovering-scroll.md)
- [comrak v0.52.0](https://github.com/kivikakk/comrak/tree/v0.52.0)
- [Aozora annotation spec](https://www.aozora.gr.jp/annotation/)
- [upstream diff policy](../../upstream/comrak/UPSTREAM_DIFF.md)
