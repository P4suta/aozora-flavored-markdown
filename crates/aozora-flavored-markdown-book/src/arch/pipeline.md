# Pipeline Overview

aozora-flavored-markdown composes three independent black boxes, each with a single
responsibility, glued together by a tiny **sentinel-stream cursor**
that keeps the two output paths (HTML and IR) in lockstep without
re-running the parser.

```text
source (UTF-8 or Shift_JIS)
   │
   ▼  aozora_encoding::decode_sjis        (Shift_JIS → UTF-8, sibling repo)
   │
   ▼  aozora_pipeline::lex_into_arena     (青空文庫記法 borrowed-AST)
   │   ├─ Phase 0  sanitize     BOM / CRLF→LF / 〔…〕 accent / PUA collision scan
   │   ├─ Phase 1  events       SIMD trigger-byte tokenise
   │   ├─ Phase 2  pair         balanced-stack bracket / ruby / quote pairing
   │   ├─ Phase 3  classify     borrowed AozoraNode<'arena> + ContainerKind
   │   └─ Phase 4  normalize    PUA sentinels (U+E001..U+E004) + Registry
   │
   │   ┌──────────────────────────── Output ────────────────────────────┐
   │   │ BorrowedLexOutput<'arena> {                                    │
   │   │     normalized: &str,                                          │
   │   │     registry: Registry<'arena>,    // sentinel pos → NodeRef   │
   │   │     diagnostics: Vec<Diagnostic>,                              │
   │   │ }                                                              │
   │   └────────────────────────────────────────────────────────────────┘
   │
   ▼  comrak::parse_document               (vanilla CommonMark + GFM)
   │   sentinels survive as plain UTF-8 — they aren't in the
   │   `<>&"'` escape set, so they stay intact in the AST.
   │
   ▼  ast_splice::splice_into_ast
   │     · replaces each sentinel node with a `Raw` node carrying
   │       aozora_render::render_node output
   │     · paragraph-aware: HeadingHint promotes to <h{level}>;
   │       sole-block-sentinel paragraphs become standalone blocks
   │     · brand boundary: aozora-* CSS classes → aozora-md-* (ADR-0011)
   │
   ▼  comrak::format_html                  (renders the spliced AST)
   │
   ▼  HTML
```

## How the splicer stays in lockstep

Both consumers of the lex output — the HTML splicer and the IR
projector — walk the **same source-order sequence** of registry
entries. The shared abstraction is `SentinelCursor` in
`crates/aozora-flavored-markdown/src/sentinel_stream.rs`:

```text
                ┌──────────────── BorrowedLexOutput ────────────────┐
                │ normalized = "前\u{E001}後..."                    │
                │ registry   = { 3 → Inline(Ruby{…}), … }           │
                └─────────────────────┬─────────────────────────────┘
                                      │
                  flatten_registry_in_source_order
                                      │
                                      ▼
             ┌──── &[NodeRef<'src>] (sorted by source pos) ─────┐
             │   [Inline(Ruby), BlockOpen(Indent), …]           │
             └──────────────────────────────────────────────────┘
                          │                            │
                          │  shared cursor             │
                ┌─────────┴────────┐         ┌─────────┴────────┐
                │ HTML splicer     │         │ IR builder       │
                │ (ast_splice)     │         │ (ir.rs)          │
                │                  │         │                  │
                │ String buffer    │         │ Vec<IrBlock>     │
                │ container_stack: │         │ container_stack: │
                │   Vec<           │         │   Vec<           │
                │     ContainerKind│         │     OpenContainer│ <- holds children
                │   >              │         │   >              │
                └──────────────────┘         └──────────────────┘
```

Both walkers consume entries linearly via `cursor.next()`, peek
ahead via `cursor.peek(offset)`, and maintain their own
container-stack so paired open / close markers nest correctly. They
never interfere because each `render` / `render_to_ir`
call materialises its own cursor over its own flattened slice.

The streaming path (`render_blocks_to_ir`) reuses this design: the
public `StreamingIrBuilder` owns the materialised slice and a
`cursor_idx` that threads across `walk_block` calls, so per-block
IR projection stays consistent with the whole-document path.

## Dependency direction

aozora-flavored-markdown depends on aozora. The reverse must not hold:

```text
┌────────────────┐      git dependency       ┌─────────────────┐
│ aozora-flavored-markdown (this repo)│ ─────────────────────────▶│ aozora (sibling)│
│   aozora-flavored-markdown │                           │  aozora-pipeline│
│   aozora-flavored-markdown-cli      │                           │  aozora-syntax  │
│   aozora-flavored-markdown-wasm     │                           │  aozora-render  │
│   aozora-flavored-markdown-book     │                           │  aozora-encoding│
└────────────────┘                           │  aozora-spec    │
                                             └─────────────────┘
```

Anything aozora-flavored-markdown needs from aozora travels through aozora's public API.
Anything aozora needs from aozora-flavored-markdown doesn't exist — by construction (see
ADR-0011 for the brand boundary that codifies this rule, and
ADR-0010 for the original split).

## What lives in the vendored comrak tree

`upstream/comrak/` is a verbatim copy of comrak v0.52.0 with a **0-line diff**
(ADR-0001). aozora-flavored-markdown composes comrak as a black box: `parse_document`, `format_html`,
and the AST type tree are imported, the sentinels survive parsing as plain
UTF-8, and the splice owns the entire aozora-md-side surface. Upgrading comrak is a
`cargo xtask upstream-sync <tag>` away — no patches to re-apply.

See [the architectural decisions](adr.md) for the full rationale (ADR-0008
zero-parser-hooks, ADR-0010 parser/renderer split, ADR-0011 brand boundary).
