# 0003. afm-parser architecture: trait-object extension, in-place inline/block hooks, arena-shared AST

- Status: accepted
- Date: 2026-04-23
- Tags: architecture, parser, api

## Context

afm-parser must:

1. Recognise Aozora Bunko constructs (ruby, bouten, 縦中横, gaiji, `［＃...］` blocks)
   *inside* comrak's CommonMark/GFM parse so that parsing interacts correctly with code
   spans, links, emphasis, and block containers (which a pre-pass cannot see).
2. Keep the upstream comrak diff ≤ 200 lines (see ADR-0001).
3. Emit rich diagnostics with source spans — which means knowing the original byte
   offset at the moment an annotation is recognised, not after later transforms.
4. Degrade gracefully: an unknown annotation becomes `AozoraNode::Annotation { kind:
   Unknown }` plus a warning, never a parse failure.
5. Be testable without exercising comrak's full parser state machine.

## Decision

### 1. Trait-object extension — follow comrak's URLRewriter pattern

Comrak already exposes extension points via `Option<Arc<dyn URLRewriter + 'c>>` and
`Option<Arc<dyn BrokenLinkCallback + 'c>>`. We add one more in the same style:

```rust
// upstream/comrak/src/parser/options.rs
pub struct Extension<'c> {
    // ...existing fields...
    pub aozora: Option<Arc<dyn afm_syntax::AozoraExtension + 'c>>,
}
```

The `AozoraExtension` trait lives in `afm-syntax` (beside `AozoraNode`), so the
extension *contract* is co-located with the AST it manipulates. afm-parser
implements the trait; comrak calls it.

### 2. Three hook points, three methods

```rust
// afm-syntax/src/extension.rs
pub trait AozoraExtension: Send + Sync + RefUnwindSafe {
    /// Called at each character-dispatch position in comrak's inline scanner.
    /// Returns the parsed node + bytes consumed, or None to let comrak continue.
    fn try_parse_inline(
        &self,
        cx: InlineCtx<'_>,
    ) -> Option<(AozoraNode, NonZeroUsize)>;

    /// Called at block-start positions. Returns how the block should be opened.
    fn try_start_block(&self, cx: BlockCtx<'_>) -> BlockDispatch;

    /// Render a recognised AozoraNode to HTML. Called from comrak's html renderer.
    fn render_html(
        &self,
        node: &AozoraNode,
        writer: &mut dyn Write,
    ) -> io::Result<()>;
}
```

- `InlineCtx` carries `&str` input, current byte offset, and a borrowed view of the
  preceding Text run (for implicit ruby-base detection). It does NOT expose
  mutable comrak state — hooks are pure from comrak's perspective.
- `BlockDispatch` is a `#[non_exhaustive]` enum: `NotOurs | Leaf(AozoraNode) |
  OpenContainer(ContainerKind) | CloseContainer`. Comrak manages the container
  stack; the trait reports classification only.
- Diagnostics accumulate inside the trait impl (it owns its own collector) and are
  harvested by afm-parser after the parse run; comrak never sees them.

Rationale: keeping comrak's diff zero-logic (pure dispatch) means upstream merges
stay a three-way auto-merge. All afm-specific reasoning lives in afm-parser.

### 3. Arena-shared AST via single NodeValue variant

Comrak stores `NodeValue` inside `Arena<AstNode>`. We add exactly one variant:

```rust
// upstream/comrak/src/nodes.rs (1 line)
NodeValue::Aozora(afm_syntax::AozoraNode),
```

All afm-specific sub-types live inside `AozoraNode`. Adding a new annotation kind
never again touches comrak.

Classifier methods (`block`, `contains_inlines`, `xml_node_name`, `accepts_lines`)
get one arm each that delegates to a method on `AozoraNode`:

```rust
NodeValue::Aozora(n) => n.is_block(),
NodeValue::Aozora(n) => n.contains_inlines(),
NodeValue::Aozora(n) => n.xml_node_name(),
NodeValue::Aozora(_) => false,
```

### 4. Ownership, spans, and numeric types

- **Input buffer**: caller-owned `&str`. afm-parser does not copy.
- **AST nodes**: own their strings as `Box<str>` (shorter than `String`, no capacity
  field — we never mutate after parse).
- **Spans**: `struct Span { pub start: u32, pub end: u32 }` — byte offsets, not char
  offsets, to match comrak. `u32` caps source size at 4 GiB, which is 4000× the
  largest plausible Aozora Bunko work; saving 8 bytes per span across thousands of
  nodes compounds.
- **Stack depth for paired blocks**: `SmallVec<[AozoraOpen; 4]>` — 99 %+ of real
  Aozora texts nest ≤ 4 deep (e.g. 字下げ inside 割り注). Heap allocation only for
  adversarial input.
- **Ruby base-run detection**: single-pass `unicode_segmentation` walk, O(n) per
  invocation — no re-scans.

### 5. Public API of afm-parser

```rust
// afm-parser/src/lib.rs
pub struct Options { /* wraps comrak::Options with aozora enabled by default */ }

pub struct ParseOutput<'a> {
    pub arena: &'a comrak::Arena<comrak::nodes::AstNode<'a>>,
    pub root: &'a comrak::nodes::AstNode<'a>,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn parse<'a>(
    arena: &'a comrak::Arena<comrak::nodes::AstNode<'a>>,
    input: &'a str,
    options: &Options,
) -> ParseOutput<'a>;

pub mod html {
    pub fn render_to_string(output: &ParseOutput<'_>, input: &str) -> String;
}
```

The arena is caller-supplied so the caller controls allocator lifetime — matches
comrak's idiom. `ParseOutput` borrows from the arena; `'a` ties everything together.

### 6. Error / diagnostic propagation

- Parse does not fail for malformed Aozora notations. It emits `AozoraNode::Annotation
  { kind: InvalidRubySpan | Unknown | ... }` plus a `Diagnostic { code, span, severity
  }`. `parse()` is infallible (apart from OOM).
- `Diagnostic` integrates `miette::Diagnostic` so the CLI gets rich span rendering
  for free.
- `tracing::warn!` events are emitted in parallel for structured-log consumers.
- CLI `--strict` mode upgrades all warnings to fatal errors after parse completes.

### 7. Module layout inside afm-parser

```
crates/afm-parser/src/
├── lib.rs              // public API: parse, Options, ParseOutput, Diagnostic
├── options.rs          // Options wrapping comrak::ExtensionOptions
├── diagnostic.rs       // Diagnostic + codes
├── adapter.rs          // impl AozoraExtension for AfmAdapter
├── html.rs             // public html::render_to_string
└── aozora/
    ├── mod.rs          // hook entrypoints shared across inline/block/html
    ├── inline.rs       // character dispatch for ｜ / 《 / 《《
    ├── ruby.rs         // ruby parser (already written; will be revised)
    ├── bouten.rs       // 《《...》》 inline + ［＃「X」に傍点］ forward-ref
    ├── block.rs        // ［＃...］ block-start classifier
    ├── block_scan.rs   // ［＃ line scanner, kind matcher (perfect-hash table)
    ├── tcy.rs          // 縦中横
    ├── gaiji.rs        // ※［＃...］ recognition + afm-encoding handoff
    ├── heading.rs      // 大/中/小 見出し aliasing → Heading normalisation
    └── html.rs         // NodeValue::Aozora → HTML rendering
```

Each file is one concern, unit-testable in isolation.

### 8. Staged implementation for M0 Spike

M0 Spike's Tier A acceptance is "parser panic ゼロ, 未消費 ［＃ ゼロ". To hit it:

1. Add `AozoraExtension` trait + `Extension.aozora` option in comrak fork
   (single ADR-0001 diff-budget contribution).
2. afm-parser ships a minimal `AfmAdapter` that:
   - recognises `｜漢字《かんじ》` and `漢字《かんじ》` → emits `AozoraNode::Ruby`
   - recognises `［＃...］` (any content) → emits `AozoraNode::Annotation {
     kind: Unknown, raw: full ［＃…］ }` for now
3. HTML renderer for ruby emits `<ruby>...</ruby>`; for Annotation emits an HTML
   comment so the raw text survives round-trip for diagnostics but is invisible.

Semantic recognition of individual annotation kinds (傍点, 字下げ, 挿絵, 割り注, …)
lands in M1–M2, incrementally. Each kind is one commit: scanner extension + HTML
renderer + fixture.

## Consequences

### Easier
- Upstream diff bounded to exactly the fields and arms listed above; future
  feature additions touch only `crates/afm-parser/src/aozora/*`.
- Unit tests for each annotation kind stand alone — no need to set up comrak parse
  state.
- Public API is stable across all milestones; callers can target `0.1.0` from M0
  and upgrade in place.
- Diagnostic spans are always correct because recognition happens during comrak's
  own parse, not in a post-pass with recomputed offsets.

### Harder
- Requires modifying comrak (tracked by 200-line budget). In exchange we get
  correctness for all edge cases (code spans, HTML blocks, raw HTML, link labels)
  because comrak's existing dispatch excludes them.
- afm-syntax now has a `RefUnwindSafe + Send + Sync` trait; we must make sure our
  adapter holds no interior mutability with panic-prone invariants.

## Alternatives considered

- **Post-processing transform**: parse with vanilla comrak, then walk the AST to
  split Text nodes. Rejected because (a) ［＃ inside code spans would require
  re-implementing comrak's context tracking in the post-pass; (b) paired block
  annotations that span paragraphs (`ここから字下げ…ここで字下げ終わり`) need
  AST rewriting at the arena level, which interacts badly with borrowing rules;
  (c) diagnostic spans drift against transformed offsets. Viable for an MVP
  preview but a dead end architecturally.
- **Typed state machine (`Parser<Ready>` / `Parser<InAozoraBlock>`)**: over-fit
  for a dispatch system that comrak already manages. The stack in
  `SmallVec<[AozoraOpen; 4]>` plus the trait's `BlockDispatch` return value
  already encodes the state transitions adequately.
- **Single giant `pub enum AozoraNode` variant with `Kind` discriminant**: less
  type-safe; chosen the per-variant approach for `#[non_exhaustive]` forward
  compatibility (new kinds don't invalidate downstream match arms).

## References

- [ADR-0001](./0001-fork-comrak-vendor-in-tree.md) — fork strategy + 200-line budget
- [ADR-0002](./0002-docker-only-execution.md) — dev env constraints
- [upstream comrak URLRewriter pattern](../../upstream/comrak/src/parser/options.rs)
- [Implementation plan](../plan.md) §4
- [Aozora annotation spec](https://www.aozora.gr.jp/annotation/)
