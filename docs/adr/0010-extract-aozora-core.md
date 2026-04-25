# 0010. Extract aozora parser core into sibling repository `aozora`

- Status: accepted
- Date: 2026-04-25
- Deciders: @P4suta
- Tags: architecture, repo-layout, release-strategy, ecosystem

## Context

ADR-0009 sketched a three-stage rollout: (1) define the stable library
surface, (2) ship sibling `aozora-tools` consuming that surface,
(3) **defer** extracting the parser into its own repo until concrete
trigger conditions held — `aozora-tools` past MVP, at least one breaking
change shipped, and a second consumer on the horizon.

`aozora-tools/docs/stage3-core-extraction.md` documented the mechanical
plan for that deferred work, naming the new repo `afm-core` and keeping
the crate names as `afm-syntax` / `afm-lexer` / `afm-parser` /
`afm-encoding`.

After living with the two-repo split for a few weeks, three things became
clear that ADR-0009 did not anticipate:

1. **The name `afm` only fits the Markdown dialect**, not the parser
   beneath it. "Aozora Flavored Markdown" is a CommonMark+GFM+aozora
   integration — a hobby Markdown dialect built on top of an aozora
   parser — and its CLI (`afm render`/`afm check`) only makes sense in
   that Markdown framing. The parser core, in contrast, has no opinion
   on Markdown; it parses 青空文庫記法 directly. Calling it `afm-*`
   conflates the dialect with the parser it sits on.
2. **`aozora-tools` already revealed the second-consumer pattern**.
   `aozora-fmt` and `aozora-lsp` reach into `afm-syntax` / `afm-lexer` /
   `afm-encoding` for diagnostics, gaiji hover, and round-trip
   formatting — not for any Markdown reason. They want the aozora layer,
   full stop. The "second non-`afm` consumer" trigger of ADR-0009 §Stage
   3 is effectively already met: the LSP and formatter are the second
   consumer, even though they ship from the sibling repo. Maintaining
   the deferral is no longer load-bearing.
3. **Naming the new repo `aozora` is honest** about what it contains.
   The user proposed it explicitly; the namespace then tells the truth:
   if it says `aozora`, it parses 青空文庫; if it says `afm`, it sits
   above the parser and adds Markdown syntax. The previous `afm-core`
   name would have continued the conflation.

## Decision

**Fast-forward Stage 3 of ADR-0009 now, with the new repo named
`aozora` (not `afm-core`) and crate names renamed wholesale to
`aozora-syntax` / `aozora-lexer` / `aozora-parser` / `aozora-encoding`
/ `aozora-corpus` / `aozora-test-utils`.** At the same time, rename the
remaining `afm-parser` crate to `afm-markdown` to make the namespace
say what it does.

### Three-layer topology after this change

```
P4suta/aozora-tools/      authoring environment (LSP/fmt/VS Code)
        │ git tag v0.1.0
        ▼
P4suta/afm/               CommonMark+GFM+aozora Markdown dialect
                          (afm-markdown, afm-cli, afm-book, vendored comrak)
        │ git tag v0.1.0
        ▼
P4suta/aozora/            pure 青空文庫記法 parser (new)
                          (aozora-syntax, aozora-lexer, aozora-parser,
                          aozora-encoding, aozora-corpus, aozora-test-utils,
                          aozora-cli [feature-gated])
```

### Naming invariant for `aozora`

The new repo's `Cargo.toml` files, source code, and doc comments contain
no mention of `comrak`, `commonmark`, `gfm`, or `markdown`. The render
seam previously expressed in terms of `comrak::HtmlFormatter` is rewritten
to use `&mut dyn io::Write`, with the comrak adapter living in
`afm-markdown` where it belongs.

### Migration approach

`git filter-repo --path-rename` preserves the history of the six
extracted crates in the new `aozora` repo, so `git blame` continues to
point at the original commits even though the crates now live one
directory level over. The `pre-aozora-split` jj bookmark on `afm` and
`aozora-tools` is a hard rollback anchor.

The rename of `afm-parser` → `afm-markdown` happens in the same
migration so that `aozora-tools`' import paths flip exactly once.

### Tag cuts

- `aozora` is born at `v0.1.0`, carrying the API surface previously
  exposed by `afm-parser` / `afm-lexer` / `afm-syntax` / `afm-encoding`
  but with renamed identifiers.
- `afm` cuts `v0.2.0` — semver-major because the workspace shape
  changed (5 crates moved out, 1 renamed).
- `aozora-tools` stays untagged (still local-only per its README).

## Consequences

**Becomes easier:**

- The namespace tells the truth. A drive-by reader of the aozora repo
  encounters no Markdown vocabulary. A drive-by reader of afm
  encounters Markdown immediately and sees aozora as a dependency.
- A future second-or-third consumer (a syntax highlighter, a Pandoc
  filter, a tree-sitter grammar derived from the lexer phases, …)
  picks the dependency that matches its scope: `aozora-parser` if it
  doesn't want CommonMark, `afm-markdown` if it does.
- Release cadence decouples: `aozora` ships on parser-correctness
  triggers (CommonMark/GFM gates do not apply); `afm` ships on
  Markdown-dialect triggers (corpus sweep is no longer afm's
  responsibility).
- The 200-line comrak diff budget (ADR-0001) and the corpus sweep
  policy (ADR-0007) live in the repo whose work they protect.

**Becomes harder:**

- Three repos must be kept consistent under tag bumps. `aozora` is
  the upstream; a breaking change on `aozora` ripples to `afm` and
  `aozora-tools`. Mitigation: the public surface contract from
  ADR-0009 carries forward unchanged on the renamed types, and the
  surface is small enough (`parse`, `serialize`, `LexOutput`,
  `Diagnostic`, `AozoraNode` and friends, `decode_sjis`,
  `gaiji::resolve`) that breakage is rare.
- `aozora-tools/docs/stage3-core-extraction.md` is now superseded;
  contributors who read the old plan get redirected by a SUPERSEDED
  header pointing at this ADR.
- `git filter-repo` rewrites history; the new repo's commit hashes
  do not match any commits visible from `afm`. Mitigation: the
  preserved history is per-file (blame works), only the SHAs change.
  Cross-references that mention specific afm commit hashes by SHA
  are rare and updated as encountered.

**Non-consequences:**

- ADR-0001's diff budget against upstream comrak is unchanged in
  spirit and (after the `aozora_syntax` token rename in
  `upstream/comrak/src/*.rs`) unchanged in line count.
- ADR-0008's zero-parser-hook architectural thesis is preserved
  verbatim — it moves to the aozora repo as that repo's foundation
  ADR-0001, with afm holding a redirect stub.
- The 17k-work corpus sweep continues to run; it moves to `aozora`
  because the invariants it checks (I1–I4, I6–I10) are aozora-layer
  properties.
- `afm-cli` keeps its existing `afm render` / `afm check` UX; the
  binary now sits above `afm-markdown` rather than `afm-parser`,
  but its options and exit codes do not change.

## Alternatives considered

**A) Honor the original ADR-0009 deferral; do not extract until the
trigger conditions hold strictly.** *Rejected.* The conditions hold in
substance — `aozora-tools` is the second consumer the ADR was waiting
for, and the dialect/parser conflation is now actively confusing. The
deferral was hedge against API churn, and the surface has stabilized
through the v0.1.0 cycle.

**B) Extract under the previously planned `afm-core` name with `afm-*`
crate names.** *Rejected.* It carries the misnomer forward into a
repo that has nothing to do with Markdown. `aozora` is shorter,
truer, and crates.io-available (verified 2026-04-25).

**C) Extract only the libraries, keep `afm-parser` as is.**
*Rejected.* Half-renaming leaves `afm-parser` as a Markdown-integration
crate with a parser-shaped name. Renaming to `afm-markdown` at the
same time costs one more `sd` pass in `aozora-tools` and yields a
namespace that explains itself.

**D) Single workspace with all three in a monorepo (Cargo workspace
+ sibling crates), no separate repos.** *Rejected.* Already weighed
in ADR-0009 §Alternative A and rejected for the same reasons:
conflated release cadence, ADR-scope creep, contributor surface
that crosses too many concerns.

## References

- ADR-0001 — fork comrak vendor + 200-line diff budget. Stays in afm.
- ADR-0008 — zero-parser-hook Aozora-first pipeline. Moves to aozora
  as ADR-0001 (foundation). afm keeps a redirect stub.
- ADR-0009 — authoring tools live in sibling repositories. The
  ecosystem ADR. Mirrored to aozora as ADR-0006.
- `aozora-tools/docs/stage3-core-extraction.md` — superseded by this
  ADR; the filter-repo path list there is reused with `--path-rename`
  hooks for the rename to `aozora-*`.
- Plan file `~/.claude/plans/markdown-flavor-aozora-flavored-markdow-groovy-stroustrup.md`
  — the ordered execution plan that this ADR captures the decision for.
