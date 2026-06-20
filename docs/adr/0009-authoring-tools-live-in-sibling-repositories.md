# 0009. Authoring tools (formatter / LSP / editor plugins) live in sibling repositories

- Status: accepted
- Date: 2026-04-25
- Tags: ecosystem, repo-layout, release-strategy

## Context

aozora-flavored-markdown's parser surface is useful beyond the `aozora-flavored-markdown render` / `aozora-flavored-markdown check` CLI:
structured diagnostics, a `parse ∘ serialize` round-trip, and gaiji resolution
make authoring tools (an `aozora-flavored-markdown fmt` formatter, an LSP, editor extensions) cheap
to build. The question is where they live — inside this workspace, or a sibling
repo.

## Decision

Authoring tools ship in a sibling repository (`P4suta/aozora-tools`), depending
on the library crates via git deps pinned to a tag. The dependency is one-way
(`aozora-tools → aozora-flavored-markdown`, never the reverse); this repo keeps no reference to it.

Precedent across the ecosystem is uniform: `taplo`, `marksman`, `texlab`, and
`rust-analyzer` all live in repos separate from the parser/compiler they wrap.

## Consequences

- Release cadence decouples: aozora-flavored-markdown ships on its CommonMark/GFM gates, aozora-tools
  on whatever testing fits formatter / LSP work.
- Contributor surface splits by skill (parser work vs. LSP / TypeScript).
- Cross-repo breaking changes need a coordinated tag bump + a downstream PR
  rather than one workspace commit; the small, stable public surface keeps this
  rare.

## Note

This ADR predates the parser extraction. ADR-0010 later moved the parser core
itself into the sibling `aozora` repo and renamed the remaining crate to
`aozora-flavored-markdown`; the library surface this ADR refers to now lives there.
