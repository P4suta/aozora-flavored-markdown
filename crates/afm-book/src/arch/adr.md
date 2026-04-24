# Architecture Decision Records

afm records load-bearing design decisions as MADR-formatted ADRs under
[`docs/adr/`](https://github.com/P4suta/afm/tree/main/docs/adr). The
rationale, alternatives considered, and the concrete consequences live
there in full — the summaries below are a map.

| #   | Title                                                 | Status                                   |
|-----|-------------------------------------------------------|------------------------------------------|
| 0001| Fork comrak in-tree, 200-line diff budget             | Accepted                                 |
| 0002| Docker-only execution for development and CI          | Accepted                                 |
| 0003| Initial afm-parser architecture                       | Parse portion superseded by 0008         |
| 0004| Accent decomposition inside `〔…〕`                    | Folded into `afm-lexer::phase0_sanitize` |
| 0005| Paired block annotation container hook                | Superseded by 0008                       |
| 0006| Lint profile policy and scope discipline              | Accepted                                 |
| 0007| 17 k-work corpus sweep strategy                       | Accepted                                 |
| 0008| Zero-parser-hook Aozora-first pipeline                | Accepted (current)                       |

New decisions follow the same format. Scaffold one with:

```sh
just adr '<title>'
```

## Why ADRs live in-repo

ADRs are part of the diff budget for upstream comrak: when a PR
touches `upstream/comrak/`, the ADR is the contract that says why.
Keeping them next to the code — and reviewable in the same PR —
means the contract evolves with the implementation.
