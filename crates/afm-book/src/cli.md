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

Render a UTF-8 file (redirect, or write straight to a file with `-o`):

```sh
afm render input.md > out.html
afm render input.md -o out.html
```

Render a Shift_JIS Aozora Bunko text directly from its published form:

```sh
afm render --encoding sjis tsumito_batsu.txt > tsumito_batsu.html
```

Pipe a document straight in from another process — `-` reads stdin
(and `--encoding sjis` applies to the piped bytes too):

```sh
cat input.md | afm render -
```

Validate a document under strict mode (treat every lexer diagnostic as
an error — useful in CI pre-flight). `--strict` exits with code 2 when a
diagnostic fires:

```sh
afm check --strict input.md
```

Diagnostics are colorized when stderr is a terminal; `--color never` or
`NO_COLOR=1` turns that off, `--color always` forces it on:

```sh
NO_COLOR=1 afm check input.md
```

See [CLI Reference](ref/cli.md) for the full flag listing, color and
verbosity rules, and exit-code semantics.
