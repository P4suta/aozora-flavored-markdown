# 0005. Paired block annotation container hook

- Status: **superseded by ADR-0008 (2026-04-24)**
- Date: 2026-04-23
- Tags: parser, upstream-diff, aozora-compat

> **Superseded.** This ADR proposed adding a paired-block container
> dispatch hook to upstream comrak's block-start loop. ADR-0008 inverts
> the architecture entirely: Aozora parsing runs in a pure-functional
> pre-pass (afm-lexer) that emits a normalized text with PUA sentinel
> characters, and a post-comrak AST walk (afm-parser::post_process)
> substitutes sentinels with AozoraNode children. Paired containers are
> now produced by the post_process block pass without any upstream
> block dispatch. The "rejected alternative B (post-process the AST)"
> below no longer applies because the block markers are pre-labeled as
> single-char sentinel lines, so post-process does not have to undo
> comrak's container splitting — comrak simply parses each marker as
> an isolated single-char paragraph that post_process pairs up.
>
> This ADR is kept for historical record. Follow ADR-0008 for the
> current design.

## Context

Aozora Bunko's paired block annotations — `［＃ここから N 字下げ］… ［＃ここで
字下げ終わり］`, `［＃割り注］… ［＃割り注終わり］`, `［＃罫囲み］…
［＃罫囲み終わり］`, `［＃ここから地付き］… ［＃ここで地付き終わり］` — delimit
**multi-line ranges of content** that need to render as a visually distinct
block (indented, right-aligned, boxed, …). The opening and closing markers
appear on their own lines, and in real corpora the inner content can include
paragraphs, line breaks, ruby, bouten, gaiji, and nested paired markers
(e.g. 字下げ inside 罫囲み).

M0 only wired comrak's inline hook; block-start dispatch was stubbed
(`BlockMatch::NotOurs` unconditional). C7 lands leaf forms (`［＃N字下げ］`,
`［＃地付き］`, `［＃地から N 字上げ］`) via the inline hook because those
markers appear mid-line and apply to a single logical line. Paired forms
don't fit that model — they delimit a *range*, and comrak's natural tree
shape for a range-with-children is a **container block**.

The 罪と罰 fixture exercises `［＃ここから２字下げ］` / `［＃ここから５字下げ］`
with `［＃ここで字下げ終わり］` closers; M2's 120-corpus test plan will
exercise all four container kinds at scale.

## Decision

Add a **paired-block container hook** to comrak's block-start loop. The hook
consults the registered `AozoraExtension` via `BlockMatch::OpenContainer(kind)`
/ `BlockMatch::CloseContainer`, and comrak drives its own container stack
accordingly — no separate stack lives on the afm side.

### Hook surface

`BlockMatch` already carries the required variants (added in M0 as
forward-looking scaffolding):

```rust
pub enum BlockMatch {
    NotOurs,
    Leaf(AozoraNode),
    OpenContainer(ContainerKind),
    CloseContainer,
}

pub enum ContainerKind {
    Indent { amount: u8 },
    Warichu,
    Keigakomi,
    AlignEnd { offset: u8 },
}
```

### Upstream diff additions

Two callers in `upstream/comrak/src/parser/mod.rs`, totalling ≤ 20 lines:

1. `handle_aozora_block(container, line) -> bool` — a new `handle_*` helper
   called from `open_new_blocks()`'s dispatch chain, mirroring the existing
   `handle_blockquote` / `handle_footnote` pattern.
2. `fn current_aozora_container(node)` — small helper that walks
   `node.ancestors()` looking for the nearest `NodeValue::Aozora(container)`
   ancestor, used by `CloseContainer` handling.

Comrak's own container stack (`container.parent()` chain) handles the rest.
`OpenContainer` calls `add_child(container, NodeValue::Aozora(kind))` and
re-points `container` at the new node; `CloseContainer` finds the nearest
Aozora-container ancestor, finalises it, and re-points `container` to its
parent.

### AST shape

`OpenContainer(kind)` maps to an existing `AozoraNode` variant per kind:

| `ContainerKind`        | `AozoraNode`                  |
|------------------------|-------------------------------|
| `Indent { amount }`    | `Indent(Indent { amount })`   |
| `AlignEnd { offset }`  | `AlignEnd(AlignEnd { offset })` |
| `Warichu`              | `Warichu(Warichu { upper, lower })` — `upper` / `lower` filled at close time from the captured inner text |
| `Keigakomi`            | `Keigakomi(Keigakomi {})`     |

For the M1 milestone, only `Indent` is promoted end-to-end. `Warichu` and
`Keigakomi` land as container scaffolding + `Annotation{Unknown}` body
capture; richer semantics follow the corpus signal in M2.

### Why reuse comrak's container-frame machinery

Comrak already tracks container blocks via the `container: &mut Node<'a>`
pointer + `parent()` traversal. Duplicating that on the afm side would:

- Double the invariant surface (two stacks must agree).
- Fight comrak on which node owns lazy-continuation, blank-line handling,
  and finalisation.
- Grow the upstream diff further at every sync, since our hook points would
  have to observe comrak's internal state transitions.

Using comrak's own machinery means the hook is a **pure classifier + tree
insertion**; everything else (lazy-continuation, trailing blank lines,
rendering order) inherits from the existing block-container code paths.

## Consequences

**What becomes easier:**

- Paired 字下げ renders as a real `<div class="afm-indent-N">` with the
  inner content as HTML children — no CSS trickery to paper over leaf
  markers.
- Nested paired forms ("字下げ inside 罫囲み") work naturally because
  comrak's container stack already handles block nesting.
- The M2 corpus can treat paired annotations as first-class AST nodes.

**What becomes harder:**

- Upstream diff grows by ~15–20 lines (169 → ~184/200). The remaining
  budget buys roughly one more hook in M2 before an ADR-level conversation
  about the 200-line cap.
- Quarterly upstream sync now has two hook points to re-apply instead of
  one. The sync is still bounded (`handle_aozora_block` is a single call
  in a single function); the cost is modest.
- Unterminated open markers need an answer: "orphan close marker" and
  "orphan open marker" cases are handled by falling back to
  `Annotation{Unknown}` so Tier-A (no bare `［＃` leaks) still holds.

**Non-consequences:**

- Inline hooks and leaf block hooks are unchanged. C7 leaf `N字下げ` /
  `地付き` / `地から N 字上げ` continue to emit via the inline dispatch;
  those stay as mid-line markers.

## Alternatives considered

**A) Leaf markers + stylesheet deduplication.** Emit each paired marker as a
leaf `Indent{amount, state: Start|End}` and let CSS use sibling selectors
(`.afm-indent-start ~ *`) to apply the block effect. *Rejected:* fragile
across nested forms, loses the natural nesting hierarchy, bakes a
CSS-specific rendering assumption into the AST. The `Warichu` upper/lower
split can't be expressed with leaves.

**B) Keep comrak untouched; post-process the AST.** After `parse_document`
returns, walk the tree and fold the leaf markers into container nodes.
*Rejected:* comrak has already split the inner content across paragraphs /
line-breaks in ways the post-pass would have to disentangle (lazy
continuation, blank-line handling). The post-pass would end up
re-implementing half of comrak's container logic.

**C) Separate afm-side container stack.** Keep the hook minimal and drive a
dedicated afm stack alongside comrak's. *Rejected:* double-book-keeping
bugs (see "Why reuse comrak's container-frame machinery").

## References

- Aozora Bunko annotation spec:
  - `docs/specs/aozora/annotation-layout_1.html` — 字下げ family
  - `docs/specs/aozora/annotation-etc.html` — 割り注, 罫囲み, 縦中横
- ADR-0001 (fork/vendor strategy) — 200-line diff budget policy.
- ADR-0003 (afm-parser architecture) — `try_start_block` + `BlockMatch`
  surface was designed in M0 for this hook.
- Plan file §M1 D1 — user-approved 2026-04-23 (option B over A).
