# CLI Quickstart

```
aozora-flavored-markdown [--encoding utf8|sjis] [--strict] <subcommand>
```

## Subcommands

| Subcommand          | Purpose                                            |
|---------------------|----------------------------------------------------|
| `aozora-flavored-markdown render <file>` | Parse and emit HTML on stdout.                     |
| `aozora-flavored-markdown check  <file>` | Parse without rendering; exit non-zero on failure. |

## Examples

Render a UTF-8 file (redirect, or write straight to a file with `-o`):

```sh
aozora-flavored-markdown render input.md > out.html
aozora-flavored-markdown render input.md -o out.html
```

Render a Shift_JIS Aozora Bunko text directly from its published form:

```sh
aozora-flavored-markdown render --encoding sjis tsumito_batsu.txt > tsumito_batsu.html
```

Pipe a document straight in from another process — `-` reads stdin
(and `--encoding sjis` applies to the piped bytes too):

```sh
cat input.md | aozora-flavored-markdown render -
```

Validate a document under strict mode (treat every lexer diagnostic as
an error — useful in CI pre-flight). `--strict` exits with code 2 when a
diagnostic fires:

```sh
aozora-flavored-markdown check --strict input.md
```

Diagnostics are colorized when stderr is a terminal; `--color never` or
`NO_COLOR=1` turns that off, `--color always` forces it on:

```sh
NO_COLOR=1 aozora-flavored-markdown check input.md
```

Generate a shell completion script (bash / zsh / fish / powershell /
elvish):

```sh
aozora-flavored-markdown completions zsh > ~/.zfunc/_aozora-flavored-markdown
```

See [CLI Reference](ref/cli.md) for the full flag listing, color and
verbosity rules, completion install paths, and exit-code semantics.
