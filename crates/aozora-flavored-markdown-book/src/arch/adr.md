# Architecture Decision Records

aozora-flavored-markdown records load-bearing design decisions as MADR-formatted ADRs under
[`docs/adr/`](https://github.com/P4suta/aozora-flavored-markdown/tree/main/docs/adr); the rationale,
alternatives, and consequences live there in full. The table below is a map.

| #    | Title                                              | Status               |
|------|----------------------------------------------------|----------------------|
| 0001 | Fork comrak in-tree, 0-line diff budget            | Accepted             |
| 0002 | Docker-only execution for development and CI       | Accepted             |
| 0004 | Accent decomposition inside `〔…〕`                | Moved to `aozora`    |
| 0006 | Lint profile policy and scope discipline           | Mirrored in `aozora` |
| 0007 | Corpus sweep strategy                              | Moved to `aozora`    |
| 0008 | Zero-parser-hook Aozora-first pipeline             | Moved to `aozora`    |
| 0009 | Authoring tools live in sibling repositories       | Accepted             |
| 0010 | Extract `aozora-*` core into a sibling repo        | Accepted             |
| 0011 | Brand boundary — `aozora-*` → `aozora-md-*` HTML rewrite | Accepted             |
| 0012 | Diagnostic JSON output schema and stability        | Accepted             |

ADRs marked **Moved** keep their number here as redirect stubs; the canonical
text lives in the sibling [`P4suta/aozora`](https://github.com/P4suta/aozora)
repo.

## What's load-bearing today

If you change anything in these areas, read the cited ADR first:

- **`upstream/comrak/`** — ADR-0001. The 0-line diff means any change here is a
  fork divergence and needs its own ADR.
- **CI / dev environment** — ADR-0002. Host toolchain is forbidden; every
  command runs through `just` + Docker.
- **Adding a new Aozora notation** — ADR-0010 + the sibling `aozora` repo. The
  lexer, AST, and per-node renderer live there now.
- **Splicing aozora output into HTML** — ADR-0008 (zero parser hooks) + ADR-0011
  (brand boundary). aozora-flavored-markdown's only aozora-md-side HTML rewrite is the `aozora-*` → `aozora-md-*`
  class pass.
- **Authoring tools** (formatter / LSP / VS Code extension) — ADR-0009 routes
  them to the sibling [`P4suta/aozora-tools`](https://github.com/P4suta/aozora-tools)
  repo.

Scaffold a new ADR with `cargo xtask new-adr '<title>'`.
