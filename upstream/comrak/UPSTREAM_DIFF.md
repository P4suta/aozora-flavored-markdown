# Upstream diff policy

This directory is a vendored copy of
[kivikakk/comrak](https://github.com/kivikakk/comrak) at tag `v0.52.0`
(SHA `60a4fae8babc3847089592868583be83d635ff1a`, see `COMRAK_SHA`).

## Rules

1. Upstream files are **verbatim** from the tagged release except for the fixed hook
   additions listed in the table below.
2. Every line modified here counts against a hard **200-line diff budget** enforced by
   `cargo xtask upstream-diff`. CI fails if the budget is exceeded.
3. All afm-specific logic lives in `crates/afm-parser/src/aozora/`, never here.
4. When comrak releases a new version, run `cargo xtask upstream-sync <tag>`. The task
   replaces this tree with the new release and re-applies the hook patches.

## Sanctioned hook points

| File                          | Addition                                                              | Max lines |
|-------------------------------|------------------------------------------------------------------------|-----------|
| `src/nodes.rs`                | `NodeValue::Aozora(afm_syntax::AozoraNode)` variant + trait arms       | ~10       |
| `src/parser/inlines.rs`       | dispatch to `aozora::inline::try_parse` on `｜` / `《`                  | ~5        |
| `src/parser/mod.rs`           | dispatch to `aozora::block::try_start` in the block-start loop         | ~5        |
| `src/html.rs`                 | render arm for `NodeValue::Aozora`                                     | ~5        |
| `src/parser/options.rs` or `lib.rs` | `ExtensionOptions.aozora: bool` flag                             | ~3        |

Note: comrak 0.52.0 uses `src/parser/inlines.rs` (plural) rather than `inline.rs`, and
keeps block-start dispatch in `src/parser/mod.rs`; the plan document references older
naming. Update the plan or adjust hook targets accordingly when the hooks land.

## Cargo.lock

The upstream `Cargo.lock` is intentionally not vendored. Our workspace owns the single
authoritative `Cargo.lock` at the repo root.
