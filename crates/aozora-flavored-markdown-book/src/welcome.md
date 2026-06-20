# Welcome

**Aozora Flavored Markdown (aozora-flavored-markdown)** is a Markdown dialect that layers
Aozora Bunko (青空文庫) typography — ruby, bouten, 縦中横, `［＃…］`
annotations, gaiji, accent decomposition — on top of CommonMark + GFM
for Japanese vertical and horizontal writing.

Like GFM, aozora-flavored-markdown is a **strict superset** of its base: any pure
CommonMark or GFM document parses identically under aozora-flavored-markdown, and the
Aozora extensions kick in only where the input actually uses them.
The file extension remains `.md`.

This handbook is both a practical tour and a reference:

- **Tour** — [install](install.md) the CLI, try the
  [CLI Quickstart](cli.md), embed the [library](library.md).
- **Reference** — walk the [parse pipeline](arch/pipeline.md), read
  the [architectural decisions](arch/adr.md), browse the
  [CLI reference](ref/cli.md) and [API reference](ref/api.md).

## Status

100% CommonMark / GFM spec compatibility, all major Aozora Bunko
annotations implemented, with a 96% regions coverage floor.

See the [project README](https://github.com/P4suta/aozora-flavored-markdown) for an
at-a-glance summary and the
[CHANGELOG](https://github.com/P4suta/aozora-flavored-markdown/blob/main/CHANGELOG.md) for
release history.
