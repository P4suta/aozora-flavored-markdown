# 0009. Authoring tools (formatter / LSP / editor plugins) live in sibling repositories

- Status: accepted; reversed in practice (see Note + ADR-0018)
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

The sibling-repo principle has since been **reversed in practice**. The `aozora`
parser absorbed the `aozora-tools` formatter/LSP/grammar back into its monorepo
(aozora ADR-0016) and archived `aozora-tools`; this workspace likewise absorbed
the EPUB generator ([ADR-0018](0018-consolidate-the-epub-generator-into-this-workspace.md)).
The cross-repo tag-bump cost this ADR judged "rare" proved routine on
tightly-coupled downstream surfaces (the EPUB themes track this renderer's
emitted classes), so co-location won. ADR-0009 stands only as the historical
rationale; new downstream tooling is consolidated here unless a concrete reason
to split it out appears.
