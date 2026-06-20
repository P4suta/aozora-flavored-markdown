# 0010. Extract aozora parser core into sibling repository `aozora`

- Status: accepted
- Date: 2026-04-25
- Tags: architecture, repo-layout, release-strategy, ecosystem

## Context

ADR-0009 deferred extracting the parser into its own repo until trigger
conditions held. Three things made extraction worth doing now:

1. The name `aozora-flavored-markdown` only fits the Markdown dialect, not the parser beneath it.
   "Aozora Flavored Markdown" is a CommonMark+GFM+aozora integration; the parser
   core has no opinion on Markdown — it parses 青空文庫記法 directly. Naming it
   `aozora-md-*` conflates the two.
2. `aozora-tools` (fmt + LSP) already consumes the aozora layer, not the Markdown
   layer — the second-consumer trigger of ADR-0009 is effectively met.
3. Naming the new repo `aozora` is honest about what it contains.

## Decision

Extract the parser into a new sibling repo `aozora`, with crates renamed
`aozora-syntax` / `-lexer` / `-parser` / `-encoding` / `-corpus` / `-test-utils`.
Rename the remaining `aozora-md-parser` crate to `aozora-flavored-markdown`. History is preserved
per-file via `git filter-repo --path-rename`.

### Three-layer topology after this change

```
P4suta/aozora-tools/   authoring environment (LSP / fmt / VS Code)
        │ git tag
        ▼
P4suta/aozora-flavored-markdown/            CommonMark+GFM+aozora Markdown dialect
                       (aozora-flavored-markdown, aozora-flavored-markdown-cli, aozora-flavored-markdown-book, vendored comrak)
        │ git tag
        ▼
P4suta/aozora/         pure 青空文庫記法 parser
                       (aozora-syntax, -lexer, -parser, -encoding, -corpus, …)
```

The `aozora` repo's Cargo.toml / source / docs name no comrak, commonmark, gfm,
or markdown; the comrak adapter lives in `aozora-flavored-markdown`.

## Consequences

- The namespace tells the truth: a reader of `aozora` meets no Markdown
  vocabulary; a reader of `aozora-flavored-markdown` meets Markdown immediately and sees aozora as a
  dependency.
- Release cadence decouples; the comrak diff budget (ADR-0001) and corpus sweep
  live in the repo whose work they protect.
- Three repos must stay consistent under tag bumps; the small public surface
  (`parse`, `serialize`, `Diagnostic`, `AozoraNode`, `decode_sjis`,
  `gaiji::resolve`) keeps breakage rare.
- ADR-0008 (zero parser hooks) moves to aozora as its foundation ADR; aozora-flavored-markdown keeps
  a redirect stub.

## References

- ADR-0001 — fork/vendor comrak. Stays in aozora-flavored-markdown.
- ADR-0008 — zero-parser-hook pipeline. Moved to aozora.
- ADR-0009 — authoring tools in sibling repos.
