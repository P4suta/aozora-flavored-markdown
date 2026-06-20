# 0011. Brand boundary: HTML class rewrite at the afm side

- Status: accepted
- Date: 2026-05-04
- Tags: rendering, css, brand, sibling-repo

## Context

afm composes two renderers: comrak (vanilla CommonMark/GFM HTML) and
`aozora-render` (the borrowed-AST 青空文庫 renderer in the sibling `aozora`
repo). aozora-render predates afm and brands its CSS classes `aozora-*`
(`aozora-ruby`, `aozora-bouten-goma`, …); the sibling mdbook theme,
`aozora-tools`, and any standalone consumer expect that prefix. afm's own public
surface uses `afm-*`, styled by the theme under `crates/afm-book/theme/`. The
two prefixes must be reconciled at the boundary.

## Decision

Reconcile on the afm side. The AST splicer
(`crates/afm-markdown/src/ast_splice.rs`) rewrites every `aozora-*` class token
in spliced aozora-render output to `afm-*` in a single linear pass; data
attributes, link targets, and text bodies are untouched.

Not parameterised upstream (a `class_prefix` knob on `aozora-render`): afm
depends on aozora, not the reverse, so aozora-render keeps its own `aozora-*`
brand for its other consumers, and the rewrite is cheap and idempotent.

## Consequences

- aozora-render stays `aozora-*` for every consumer; afm's public HTML carries
  only `afm-*`, pinned by the `AFM_CLASSES` contract in `afm-markdown-test-support`
  and the `property_html_shape` sweep.
- New `aozora-*` classes are picked up automatically as long as the prefix holds.
- This is the only afm-side rewrite of upstream HTML (besides the Tier-A `［＃`
  wrapper for unclaimed annotations); adding more needs a follow-up ADR.
