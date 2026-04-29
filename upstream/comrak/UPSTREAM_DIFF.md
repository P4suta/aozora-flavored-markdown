# Upstream diff policy

This directory is a vendored copy of
[kivikakk/comrak](https://github.com/kivikakk/comrak) at tag `v0.52.0`
(SHA `60a4fae8babc3847089592868583be83d635ff1a`, see `COMRAK_SHA`).

## Rules

1. Upstream files are **verbatim** from the tagged release. The vendored tree
   carries **zero Aozora-aware additions** since v0.2.4 (afm Phase F.5).
2. The historical 200-line diff budget (ADR-0001) has been collapsed to
   **0 lines**. afm-markdown lives entirely outside this directory and
   composes comrak as a black box.
3. All afm-specific logic lives in `crates/afm-markdown/`, never here.
4. When comrak releases a new version, run `cargo xtask upstream-sync <tag>`.
   The task replaces this tree with the new release; no patch reapplication
   is required because there are no patches.

## How afm composes comrak (no hook points)

| Stage | Crate | Operation |
|-------|-------|-----------|
| Lex | `aozora-lex` | Replace 青空文庫記法 spans with PUA sentinels (`U+E001..U+E004`) and stash the `Registry`. |
| Parse | `comrak` (this tree) | Run `parse_document` against the normalized text. Sentinels flow through as plain UTF-8 (not in CommonMark's escape set). |
| Format | `comrak` (this tree) | Run `format_html` against the AST. Sentinels survive into the output verbatim. |
| Splice | `afm-markdown::post_process` | Walk the produced HTML; substitute each sentinel with `aozora-render::render_node`'s output and rewrite block-sentinel paragraphs into container HTML. |

The substitution is order-based: sentinels appear in the formatted HTML in
the same order the lexer wrote them into `normalized`, so we pre-flatten the
registry into an ordered `Vec<NodeRef<'_>>` and dispatch sequentially.

## Cargo.lock

The upstream `Cargo.lock` is intentionally not vendored. Our workspace owns
the single authoritative `Cargo.lock` at the repo root.

## History

The pre-v0.2.4 tree carried these patches (~22 lines on a 200-line budget):

- `src/nodes.rs` — `NodeValue::Aozora(Box<aozora_syntax::AozoraNode>)` variant
- `src/html.rs` — render arm + `render_aozora` `fn` pointer dispatch
- `src/cm.rs` / `src/xml.rs` — placeholder arms
- `src/parser/options.rs` — `render_aozora` / `serialize_aozora` `fn` pointer fields
- `src/tests/sourcepos.rs` — exclusion of the `Aozora` variant from the harness

All of those were removed when aozora 0.2.0 retired the owned-AST
`aozora-parser` crate (Phase F.1) and afm-markdown re-implemented the
integration on top of the borrowed-AST renderer in `aozora-render`. See
ADR-0010 for the parser-core extraction that triggered this rewrite, and
the `crates/afm-markdown/src/post_process.rs` module-level docs for the
substitution algorithm.
