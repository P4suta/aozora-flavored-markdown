# Architecture Decision Records

afm records load-bearing design decisions as MADR-formatted ADRs
under
[`docs/adr/`](https://github.com/P4suta/afm/tree/main/docs/adr). The
rationale, alternatives considered, and concrete consequences live
there in full — the table below is a map.

| #    | Title                                            | Status                                  |
|------|--------------------------------------------------|-----------------------------------------|
| 0001 | Fork comrak in-tree, **0-line diff budget**      | Accepted (budget collapsed in v0.2.4)   |
| 0002 | Docker-only execution for development and CI     | Accepted                                |
| 0003 | Initial afm-parser architecture                  | Superseded by ADR-0010 (v0.2.0 split)   |
| 0004 | Accent decomposition inside `〔…〕`              | Moved to sibling `aozora` repo          |
| 0005 | Paired block annotation container hook           | Superseded by ADR-0008                  |
| 0006 | Lint profile policy and scope discipline         | Mirrored in sibling `aozora` repo       |
| 0007 | 17 k-work corpus sweep strategy                  | Moved to sibling `aozora` repo          |
| 0008 | Zero-parser-hook Aozora-first pipeline           | Moved to sibling `aozora` repo          |
| 0009 | Authoring tools live in sibling repositories     | Accepted                                |
| 0010 | Extract `aozora-*` core into a sibling repo      | Accepted (executed v0.2.0, 2026-04-25)  |
| 0011 | Brand boundary — `aozora-*` → `afm-*` HTML rewrite | Accepted (2026-05-04)                |

ADRs marked **Moved** kept their number on this side as redirect
stubs (e.g. `0008-MOVED.md`); the canonical text now lives in the
sibling [`P4suta/aozora`](https://github.com/P4suta/aozora) repo.

## What's load-bearing today

If you change anything in these areas, read the cited ADR first:

- **`upstream/comrak/`** — ADR-0001. 0-line diff means any change
  here is a fork divergence and needs its own ADR.
- **CI / dev environment** — ADR-0002. Host toolchain is forbidden;
  every command runs through `just` + Docker.
- **Adding a new Aozora notation** — ADR-0010 + the sibling
  `aozora` repo's CLAUDE.md. The lexer, AST, and per-node renderer
  all live there now.
- **Splicing aozora output into HTML** — ADR-0008 (zero parser
  hooks) + ADR-0011 (brand boundary). afm's only afm-side rewrite
  of upstream HTML is the `aozora-*` → `afm-*` class pass.
- **Authoring tools** (formatter / LSP / VS Code extension) —
  ADR-0009 routes them to the sibling
  [`P4suta/aozora-tools`](https://github.com/P4suta/aozora-tools)
  repo.

New decisions follow the same MADR format. Scaffold one with:

```sh
cargo xtask new-adr '<title>'
```

## Why ADRs live in-repo

ADRs are part of the diff budget for upstream comrak: when a PR
touches `upstream/comrak/`, the ADR is the contract that says why.
Keeping them next to the code — and reviewable in the same PR —
means the contract evolves with the implementation.
