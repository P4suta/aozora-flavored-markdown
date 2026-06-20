# CLI Reference

The up-to-date reference is `afm --help` / `afm <subcommand> --help`.
The pages below mirror the same information for offline browsing.

## `afm`

```
afm [--encoding utf8|sjis] [--strict] <subcommand> [<args>]
```

### Global flags

| Flag                 | Default | Effect                                                       |
|----------------------|---------|--------------------------------------------------------------|
| `--encoding <enc>`   | `utf8`  | Input encoding. `utf8` or `sjis`.                            |
| `--strict`           | off     | Promote every lexer diagnostic to a hard error (exit 2).     |
| `--help`             | —       | Print help and exit.                                         |
| `--version`          | —       | Print version and exit.                                      |

### Exit codes

| Code | Meaning                                                  |
|------|----------------------------------------------------------|
| 0    | Success.                                                 |
| 1    | Generic error (I/O, invalid flag, …).                    |
| 2    | Lexer / parser diagnostic in `--strict` mode.            |

## `afm render <input>`

Parse `<input>` and write HTML on stdout.

```sh
afm render input.md > out.html
```

`<input>` may be `-` to read from stdin.

## `afm check <input>`

Parse `<input>` without emitting HTML. Useful for CI pre-flight.

```sh
afm check --strict input.md
```

Exits non-zero on parse errors or — under `--strict` — on any lexer
diagnostic.
