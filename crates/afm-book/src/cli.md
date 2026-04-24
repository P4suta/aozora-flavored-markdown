# CLI Quickstart

```
afm [--encoding utf8|sjis] [--strict] <subcommand>
```

## Subcommands

| Subcommand          | Purpose                                            |
|---------------------|----------------------------------------------------|
| `afm render <file>` | Parse and emit HTML on stdout.                     |
| `afm check  <file>` | Parse without rendering; exit non-zero on failure. |

## Examples

Render a UTF-8 file:

```sh
afm render input.md > out.html
```

Render a Shift_JIS Aozora Bunko text directly from its published form:

```sh
afm render --encoding sjis tsumito_batsu.txt > tsumito_batsu.html
```

Validate a document under strict mode (treat every lexer diagnostic as
an error — useful in CI pre-flight):

```sh
afm check --strict input.md
```

See [CLI Reference](ref/cli.md) for the full flag listing and exit-code
semantics.
