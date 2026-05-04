# Pipeline Overview

afm composes three independent black boxes, each with a single
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
   │   `<>&"'` escape set, so format_html passes them through too.
   │
   ▼  comrak::format_html                  (HTML with sentinels in body)
   │
   ▼  afm_markdown::post_process::splice_aozora_html
   │     · single-pass scan over the emitted HTML
   │     · sentinel ↔ aozora_render::render_node output substitution
   │     · paragraph-aware: HeadingHint promotes to <h{level}>;
   │       sole-block-sentinel paragraphs become standalone blocks
   │     · brand boundary: aozora-* CSS classes → afm-* (ADR-0011)
   │
   ▼  HTML
```

## How the splicer stays in lockstep

Both consumers of the lex output — the HTML splicer and the IR
projector — walk the **same source-order sequence** of registry
entries. The shared abstraction is
[`SentinelCursor`](https://p4suta.github.io/afm/api/afm_markdown/sentinels/struct.SentinelCursor.html)
in `crates/afm-markdown/src/sentinels.rs`:

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
                │ (post_process)   │         │ (ir.rs)          │
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
never interfere because each `render_to_string` / `render_to_ir`
call materialises its own cursor over its own flattened slice.

The streaming path (`render_blocks_to_ir`) reuses this design: the
public `StreamingIrBuilder` owns the materialised slice and a
`cursor_idx` that threads across `walk_block` calls, so per-block
IR projection stays consistent with the whole-document path.

## Dependency direction

afm depends on aozora. The reverse must not hold:

```text
┌────────────────┐      git dependency       ┌─────────────────┐
│ afm (this repo)│ ─────────────────────────▶│ aozora (sibling)│
│   afm-markdown │                           │  aozora-pipeline│
│   afm-cli      │                           │  aozora-syntax  │
│   afm-wasm     │                           │  aozora-render  │
│   afm-book     │                           │  aozora-encoding│
└────────────────┘                           │  aozora-spec    │
                                             └─────────────────┘
```

Anything afm needs from aozora travels through aozora's public API.
Anything aozora needs from afm doesn't exist — by construction (see
ADR-0011 for the brand boundary that codifies this rule, and
ADR-0010 for the original split).

## What lives in the vendored comrak tree

`upstream/comrak/` is a verbatim copy of comrak v0.52.0 with a
**0-line diff** (ADR-0001 v0.2.4). afm composes comrak as a black
box: `parse_document`, `format_html`, and the AST type tree are
imported, the sentinels survive both passes as plain UTF-8, and
post-process owns the entire afm-side surface. Upgrading comrak is
a `cargo xtask upstream-sync <tag>` away — no patches to re-apply.

See [the architectural decisions](adr.md) for the full rationale and
the alternatives that led here (ADR-0008 reset the design to
zero-parser-hooks; ADR-0010 split parser / renderer into the sibling
repo; ADR-0011 nailed down the brand boundary).
