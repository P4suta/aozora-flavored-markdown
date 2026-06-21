# Library Usage

aozora-flavored-markdown ships as a Rust library (`aozora-flavored-markdown`) alongside the CLI. The
binary is a thin wrapper over the same public API every embedder
calls — there is no parallel "library-only" path that the CLI
bypasses, so a CLI run and a library embed produce byte-identical
HTML for the same input.

## Add the dependency

aozora-flavored-markdown is not on crates.io yet; depend on it directly by git URL:

```toml
[dependencies]
aozora-flavored-markdown = { git = "https://github.com/P4suta/aozora-flavored-markdown" }
```

The `aozora` sibling crate provides Shift_JIS decoding through its
`aozora::encoding` module when you need it; it is published on crates.io:

```toml
[dependencies]
aozora = "0.4.1"
```

## Render to HTML — the simple path

```rust
use aozora_flavored_markdown::{Options, render};

fn main() {
    let rendered = render(
        "彼は｜青梅《おうめ》に行った。",
        &Options::default(),
    );

    println!("{}", rendered.html);
    for diag in &rendered.diagnostics {
        eprintln!("warning: {diag}");
    }
}
```

`Options::default()` enables the GFM extensions aozora-flavored-markdown uses on top
of CommonMark (strikethrough, tables, autolinks, task lists),
hardbreaks (so each Aozora source newline becomes a `<br>` — verse /
dialogue boundaries are load-bearing in 青空文庫 source), and the
Aozora pre-pass.

For pure CommonMark or pure GFM behaviour (no Aozora recognition),
use `Options::commonmark_only()` or `Options::gfm_only()` — these are
also what the CommonMark 0.31.2 and GFM 0.29 spec runners exercise.

## Render to a structured IR

`render_to_ir` returns the same HTML alongside a typed `IrDocument`
that mirrors the TypeScript `IRDocument` consumed by aozora-flavored-markdown-obsidian:

```rust
use aozora_flavored_markdown::ir::{IrBlock, IrInline};
use aozora_flavored_markdown::{Options, render_to_ir};

fn main() {
    let rendered = render_to_ir(
        "# 第一章\n\n｜青梅《おうめ》",
        &Options::default(),
    );

    for block in &rendered.ir.blocks {
        match block {
            IrBlock::Heading { level, .. } => println!("h{level}"),
            IrBlock::Paragraph { children, .. } => {
                let ruby_count = children
                    .iter()
                    .filter(|c| matches!(c, IrInline::Ruby { .. }))
                    .count();
                println!("paragraph with {ruby_count} ruby span(s)");
            }
            other => println!("{other:?}"),
        }
    }
}
```

The IR carries every Aozora-side construct (`Ruby`, `DoubleRuby`,
`Bouten`, `Tcy`, `Gaiji`, `Annotation`, `Container`, `PageBreak`,
`SectionBreak`) plus the markdown-side block / inline shapes — so
JS-side renderers in aozora-flavored-markdown-obsidian / aozora-flavored-markdown-logseq can pick their own
output target (DOM fragment, CodeMirror RangeSet, semantic tokens)
without re-parsing the HTML.

## Render block-by-block (streaming)

For long documents where you want to checkpoint between blocks
(aozora-flavored-markdown-obsidian uses this for `AbortSignal` cancellation in chunked
post-processors), use `render_blocks_to_ir`:

```rust
use aozora_flavored_markdown::{Options, render_blocks_to_ir};

let (blocks, diagnostics) = render_blocks_to_ir(
    "first paragraph\n\n｜second《せかんど》paragraph",
    &Options::default(),
);

for block in blocks {
    println!("{} ir nodes at line {}", block.ir.len(), block.source_line);
    println!("{}", block.html);
}
assert!(diagnostics.is_empty());
```

The shared `StreamingIrBuilder` threads the sentinel cursor across
calls, so per-block IR projection stays in lockstep with the
whole-document path. A block may carry zero IR entries (e.g.
container-open paragraphs that drain at the next call boundary) or
more than one (a container that finally closes).

## Reading Shift_JIS input

Aozora Bunko ships its text files in Shift_JIS. `aozora::encoding`
exposes a transparent decoder so your pipeline doesn't need to know
the encoding ahead of time:

```rust
use aozora_flavored_markdown::{Options, render};
use aozora::encoding::decode_sjis;

fn main() -> std::io::Result<()> {
    let bytes = std::fs::read("tsumito_batsu.txt")?;
    let utf8 = decode_sjis(&bytes).expect("decoded");

    let rendered = render(&utf8, &Options::default());
    std::fs::write("tsumito_batsu.html", rendered.html)?;
    Ok(())
}
```

## Round-tripping through the lexer

`aozora_flavored_markdown::serialize` is the inverse of the lex pre-pass: it
replays the borrowed-AST registry to reconstruct the original aozora-flavored-markdown
markup byte-for-byte (modulo the lexer's Phase-0 sanitisation). This
is what the upstream 17 k-work corpus sweep exercises as I3 (round-
trip fixed point):

```rust
use aozora_flavored_markdown::serialize;

fn main() {
    let source = "彼は｜青梅《おうめ》に行った。";
    assert_eq!(serialize(source), source);
}
```

## More examples

End-to-end snippets live under
[`crates/aozora-flavored-markdown/examples/`](https://github.com/P4suta/aozora-flavored-markdown/tree/main/crates/aozora-flavored-markdown/examples)
in the repository:

- `render-utf8.rs` — UTF-8 source → HTML on stdout.
- `render-sjis.rs` — Shift_JIS source via `aozora::encoding`.
- `ast-walk.rs` — walk the parsed AST and tally `AozoraNode`
  variants.
- `serialize-round-trip.rs` — verify `serialize ∘ lex ≡ id` on one
  file.

Run any of them with:

```sh
cargo run --example <name> -p aozora-flavored-markdown -- <path>
```
