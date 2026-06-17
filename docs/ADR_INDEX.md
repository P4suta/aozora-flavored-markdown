# Architecture Decision Records

This directory holds [MADR 4.0](https://adr.github.io/madr/) Architecture
Decision Records. Each file documents one decision; once accepted an ADR is
never edited — it is *superseded* by a later ADR that links back.

Several early ADRs **moved** to the sibling [`P4suta/aozora`](https://github.com/P4suta/aozora)
repo when the parser core was extracted (ADR-0010); their numbers are kept as
redirect stubs here so existing links don't rot.

| ADR                                                          | Title                                                              | Status                        |
| ------------------------------------------------------------ | ------------------------------------------------------------------ | ----------------------------- |
| [0001](./adr/0001-fork-comrak-vendor-in-tree.md)             | Fork comrak and vendor it in-tree (0-line diff budget)             | accepted                      |
| [0002](./adr/0002-docker-only-execution.md)                  | Every dev operation runs inside Docker                             | accepted                      |
| [0003](./adr/0003-afm-parser-architecture.md)                | afm-parser architecture (trait-object extension, arena AST)        | superseded by 0010            |
| [0004](./adr/0004-MOVED.md)                                  | → `aozora/docs/adr/0003-accent-decomposition-preparse.md`          | moved                         |
| [0005](./adr/0005-paired-block-container-hook.md)            | Paired block annotation container hook                             | superseded by 0008            |
| [0006](./adr/0006-MOVED.md)                                  | mirrored → `aozora/docs/adr/0004-lint-profile-policy.md`           | moved                         |
| [0007](./adr/0007-MOVED.md)                                  | → `aozora/docs/adr/0005-corpus-sweep-strategy.md`                  | moved                         |
| [0008](./adr/0008-MOVED.md)                                  | → `aozora/docs/adr/0001-zero-parser-hooks.md`                      | moved                         |
| [0009](./adr/0009-authoring-tools-live-in-sibling-repositories.md) | Authoring tools live in sibling repositories                 | accepted                      |
| [0010](./adr/0010-extract-aozora-core.md)                    | Extract aozora parser core into sibling repository `aozora`        | accepted                      |
| [0011](./adr/0011-brand-boundary-css-class-rewrite.md)       | Brand boundary: HTML class rewrite at the afm side                 | accepted                      |

## Authoring a new ADR

1. Scaffold with `cargo xtask new-adr 'my new decision'` (copies
   `adr/0000-template.md` to the next sequential number).
2. Fill in the sections; keep paragraphs short and action-oriented.
3. Add a row to the table above.
4. Reference the ADR in the commit body and open a PR. ADRs are normally
   accepted on merge; controversial ones land as `proposed` and flip to
   `accepted` once the discussion concludes.
