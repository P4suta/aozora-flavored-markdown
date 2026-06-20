# CLI Reference

The up-to-date reference is `afm --help` / `afm <subcommand> --help`.
The pages below mirror the same information for offline browsing.

## `afm`

```
afm [--encoding utf8|sjis] [--strict] [--color auto|always|never] [-v|-q] <subcommand> [<args>]
```

### Global flags

| Flag                 | Default | Effect                                                       |
|----------------------|---------|--------------------------------------------------------------|
| `--encoding <enc>`   | `utf8`  | Input encoding. `utf8` or `sjis`.                            |
| `--strict`           | off     | Promote every lexer diagnostic to a hard error (exit 2).     |
| `--color <when>`     | `auto`  | Colorize diagnostics: `auto`, `always`, or `never`.          |
| `-v`, `--verbose`    | —       | Raise log verbosity (`-v` info, `-vv` debug, `-vvv` trace).  |
| `-q`, `--quiet`      | —       | Lower log verbosity (`-q` errors only).                      |
| `--help`             | —       | Print help and exit.                                         |
| `--version`          | —       | Print version and exit.                                      |

### Exit codes

| Code | Meaning                                                  |
|------|----------------------------------------------------------|
| 0    | Success.                                                 |
| 1    | Generic error (I/O, invalid flag, …).                    |
| 2    | Lexer / parser diagnostic in `--strict` mode.            |

### Color

`--color always` / `--color never` force colorized error reports on or
off. Under the default `--color auto`, the choice follows, in order:

1. `NO_COLOR` (set, any value) — disables color.
2. `CLICOLOR_FORCE` (set, not `0`) — forces color.
3. otherwise, color is on only when stderr is a terminal.

An explicit `--color always`/`never` wins over the environment.

### Verbosity

`-v`/`-q` set the default tracing level for the run (logs go to stderr).
A `RUST_LOG` environment variable, when set, overrides `-v`/`-q`
entirely.

## `afm render <input>`

Parse `<input>` and write HTML on stdout (or to `--output`).

```sh
afm render input.md > out.html
afm render input.md -o out.html
```

`<input>` may be `-` to read from stdin.

| Flag                 | Default  | Effect                                          |
|----------------------|----------|-------------------------------------------------|
| `-o`, `--output <p>` | stdout   | Write HTML to `<p>` instead of stdout (`-` = stdout). |

## `afm check <input>`

Parse `<input>` without emitting HTML. Useful for CI pre-flight.

```sh
afm check --strict input.md
```

Exits non-zero on parse errors or — under `--strict` — on any lexer
diagnostic.
