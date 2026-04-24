# Pipeline Overview

afm's parse pipeline is a three-stage funnel: Aozora notations are
fully resolved **before** comrak runs, and spliced back into the AST
afterwards. The vendored comrak tree has zero parse-time hooks for
Aozora — its recognition lives entirely in `afm-lexer` and an
`afm-parser` post-process.

```text
source (UTF-8 or Shift_JIS)
   │
   ▼  afm-encoding::decode          (if SJIS)
   │
   ▼  afm-lexer::lex                (7-phase pure function)
   │    0 sanitize   — BOM / CRLF→LF / 〔…〕 accent / PUA collision scan
   │    1 events     — linear tokenise of Aozora trigger glyphs
   │    2 pair       — balanced-stack pairing of brackets, quotes, ruby
   │    3 classify   — per-shape → AozoraNode + ContainerKind
   │    4 normalize  — replace each Aozora span with a PUA sentinel
   │    5 registry   — sentinel → AozoraNode lookup table
   │    6 validate   — V1-V3 structural invariants
   │
   ▼  comrak::parse_document         (vanilla CommonMark + GFM)
   │
   ▼  afm-parser::post_process       (AST splice)
   │    • splice_inline — Text with U+E001 → Aozora(...)
   │    • splice_block_leaf — Paragraph[U+E002] → Aozora block node
   │    • splice_paired_container — U+E003/U+E004 → Container wrap
   │
   ▼  comrak HTML renderer (+ NodeValue::Aozora → afm render fn)
   │
   ▼  HTML
```

The full-round-trip path reuses the lexer's registry as its inverse
input: a single O(n) byte sweep walks the lexer's normalised text and
substitutes each PUA sentinel with the original afm markup, so
`serialize ∘ parse ≡ id` on the lexer's normalised input.

See [ADR-0008](adr.md) for the full design rationale and the list of
alternative architectures considered.
