# Library Usage

afm is a Rust library as well as a binary. Everything the CLI does is
layered on top of public APIs on `afm-parser`, `afm-syntax`, and
`afm-encoding`.

## A minimal parse + render

```rust
use comrak::Arena;

fn main() {
    let arena = Arena::new();
    let options = afm_parser::Options::default();
    let input = "｜青梅《おうめ》に行った。";

    let parsed = afm_parser::parse(&arena, input, &options);
    let html = afm_parser::render_to_string(&parsed);

    println!("{html}");
}
```

## Reading Shift_JIS input

Aozora Bunko ships its text files in Shift_JIS. `afm-encoding` exposes
a transparent decoder so your pipeline doesn't need to know the
encoding ahead of time:

```rust
use afm_encoding::decode_sjis;
use comrak::Arena;

fn main() -> anyhow::Result<()> {
    let bytes = std::fs::read("tsumito_batsu.txt")?;
    let utf8 = decode_sjis(&bytes)?;

    let arena = Arena::new();
    let options = afm_parser::Options::default();
    let parsed = afm_parser::parse(&arena, &utf8, &options);
    let html = afm_parser::render_to_string(&parsed);

    std::fs::write("tsumito_batsu.html", html)?;
    Ok(())
}
```

## Round-tripping through the AST

`afm_parser::serialize` is the inverse of `parse`, replaying the lexer
registry to reconstruct the original afm markup byte-for-byte (modulo
the lexer's normalisation). This is what I3 (round-trip fixed point)
in the 17 k-work corpus sweep exercises.

```rust
use comrak::Arena;

fn main() {
    let arena = Arena::new();
    let options = afm_parser::Options::default();
    let parsed = afm_parser::parse(&arena, "｜青梅《おうめ》", &options);

    let roundtripped = afm_parser::serialize(&parsed);
    assert_eq!(roundtripped, "｜青梅《おうめ》");
}
```

## More examples

See the [`examples/`](https://github.com/P4suta/afm/tree/main/examples)
directory in the repository for UTF-8 render, Shift_JIS render, AST
walking, and round-trip serialisation reference snippets.
