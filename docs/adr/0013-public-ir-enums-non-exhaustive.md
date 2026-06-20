# 0013. Public IR enums are `#[non_exhaustive]`

- Status: accepted
- Date: 2026-06-20
- Deciders: @P4suta
- Tags: api, ir, stability, semver

## Context

`afm_markdown::ir::{IrBlock, IrInline}` are the public, `serde`-serialised IR
the wasm bridge hands to afm-obsidian's TypeScript renderers. They grow over
time: every new 青空文庫 notation that lands upstream surfaces here as a new
variant (the IR already carries `Ruby`, `DoubleRuby`, `Bouten`, `Tcy`,
`Gaiji`, `Annotation`, `Container`, `PageBreak`, `SectionBreak`, …).

With afm-markdown heading for crates.io (ADR-0015), external Rust consumers
become possible. If the enums were exhaustive, every added variant would be a
**breaking** change for any downstream `match` — forcing a major (pre-1.0:
minor) bump for what is conceptually an additive feature.

A complication: the TypeScript union in `crates/xtask/src/types.rs` is
hand-written, kept honest by `assert_*_variants` exhaustive matches that fail
to compile when a variant is added. `#[non_exhaustive]` forbids an exhaustive
match *from another crate*, which is where those witnesses lived.

## Decision

Mark `IrBlock` and `IrInline` `#[non_exhaustive]`. External Rust consumers must
add a `_ =>` arm; afm can then introduce a new IR variant in a
minor/patch release without breaking them. Serde output is unchanged, so the
JSON/TypeScript contract is unaffected (TS consumers already tolerate an
unknown `kind`, matching the ADR-0012 "tolerate unknown code" rule).

Relocate the `assert_block_variants` / `assert_inline_variants` completeness
witnesses **into the owning crate** (`afm_markdown::ir::types`), where an
exhaustive match is still allowed. They keep forcing a compile error on a new
variant; their comment is the reminder to also extend the hand-written `.d.ts`
union and its samples in `crates/xtask/src/types.rs`.

The IR **structs** (`IrDocument`, `IrTableRow`, `IrListItem`, `IrDiagnostic`)
and `IrTableAlign`, `Range`, `Position` are deliberately left exhaustive:
`IrTableAlign` is a closed GFM set, `Range`/`Position` are a stable coordinate
contract, and the structs are constructed by literal in the xtask
field/tag-completeness samples. Their fields already evolve additively via
`#[serde(skip_serializing_if = "Option::is_none")]` optional fields, so the
wire contract stays additive without `#[non_exhaustive]`.

## Consequences

- Adding an Aozora IR variant is no longer a breaking change for external Rust
  consumers — only the in-crate witness must be updated (which also nudges the
  TS union).
- Downstream Rust `match`es over `IrBlock`/`IrInline` now require a wildcard
  arm. afm's own walker (`ir/mod.rs`) and the relocated witnesses are in-crate
  and unaffected.
- Struct field additions remain breaking for out-of-crate *literal*
  construction, but the only such constructors are the in-workspace xtask
  samples, so this costs nothing today and is revisited if an external builder
  appears.

## Alternatives considered

**Leave the enums exhaustive.** Keeps the cross-crate xtask witness simple, but
makes every future notation a breaking change for published-crate consumers —
the exact churn `#[non_exhaustive]` exists to avoid.

**Mark the structs `#[non_exhaustive]` too.** Maximises forward-compat but
breaks the xtask field/tag-completeness samples (a non_exhaustive struct can't
be built by literal from another crate), which would force builders or sample
relocation for no current benefit. Deferred until an external constructor
exists.

## References

- ADR-0012 (diagnostic JSON schema & stability — additive-only precedent)
- ADR-0015 (crates.io publication & semver policy)
- `crates/afm-markdown/src/ir/types.rs`, `crates/xtask/src/types.rs`
- Plan: `~/.claude/plans/aozora-dapper-hopper.md`
