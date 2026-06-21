# CLI Reference

The up-to-date reference is `aozora-flavored-markdown --help` / `aozora-flavored-markdown <subcommand> --help`.
The pages below mirror the same information for offline browsing.

## `aozora-flavored-markdown`

```
aozora-flavored-markdown [--encoding utf8|sjis] [--strict] [--color auto|always|never] [-v|-q] <subcommand> [<args>]
```

### Global flags

| Flag                 | Default | Effect                                                       |
|----------------------|---------|--------------------------------------------------------------|
| `--encoding <enc>`   | `utf8`  | Input encoding. `utf8` or `sjis`.                            |
| `--strict`           | off     | Promote every lexer diagnostic to a hard error (exit 2).     |
| `--color <when>`     | `auto`  | Colorize diagnostics: `auto`, `always`, or `never`.          |
| `--format <fmt>`     | `human` | Diagnostic format: graphical `human` or stable `json`.       |
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

### Human diagnostics

The default `--format human` renders each diagnostic as a graphical block
(rustc/clippy style) on **stderr** — the severity, the stable `aozora::…`
code, the message, and a source snippet with a caret under the offending
span:

```
aozora::lex::unmatched_close

  × unmatched Aozora Ruby close delimiter
   ╭─[<stdin>:1:7]
 1 │ orphan》close
   ·       ──
   ╰────
```

Colorization follows [`--color`](#color). For machine consumption use
`--format json` instead.

### JSON diagnostics

`--format json` emits diagnostics as a stable `aozora-md.diagnostics.v1`
envelope for tooling (editors, CI gates, LSP bridges):

```json
{
  "schema": "aozora-md.diagnostics.v1",
  "diagnostics": [
    {
      "code": "aozora::lex::unmatched_close",
      "severity": "error",
      "source": "source",
      "message": "…",
      "span": { "start": 6, "end": 9 },
      "line": 1,
      "column": 7
    }
  ]
}
```

- `code`, `severity` (`error`/`warning`/`note`), and `source`
  (`source`/`internal`) are stable identifiers — key on these.
- `span` holds byte offsets; `line`/`column` are 1-based (column counts
  characters).
- `message` is human text and is **not** part of the contract.
- The envelope is emitted even when there are no diagnostics.

`check --format json` writes to **stdout** (so it pipes into `jq`);
`render --format json` keeps stdout for HTML and writes the JSON to
**stderr**. Stability is additive-only within `v1`; see
[ADR-0012](https://github.com/P4suta/aozora-flavored-markdown/blob/main/docs/adr/0012-diagnostic-json-output-schema-and-stability.md).

```sh
aozora-flavored-markdown check --format json input.md | jq '.diagnostics[].code'
```

## `aozora-flavored-markdown render <input>`

Parse `<input>` and write HTML on stdout (or to `--output`).

```sh
aozora-flavored-markdown render input.md > out.html
aozora-flavored-markdown render input.md -o out.html
```

`<input>` may be `-` to read from stdin.

| Flag                 | Default  | Effect                                          |
|----------------------|----------|-------------------------------------------------|
| `-o`, `--output <p>` | stdout   | Write HTML to `<p>` instead of stdout (`-` = stdout). |

## `aozora-flavored-markdown check <input>`

Parse `<input>` without emitting HTML. Useful for CI pre-flight.

```sh
aozora-flavored-markdown check --strict input.md
```

Exits non-zero on parse errors or — under `--strict` — on any lexer
diagnostic.

## `aozora-flavored-markdown completions <shell>`

Print a shell completion script on stdout for `bash`, `zsh`, `fish`,
`powershell`, or `elvish`. Install it wherever your shell looks for
completions, for example:

```sh
# bash
aozora-flavored-markdown completions bash | sudo tee /etc/bash_completion.d/aozora-flavored-markdown > /dev/null
# zsh (a directory on your $fpath)
aozora-flavored-markdown completions zsh > ~/.zfunc/_aozora-flavored-markdown
# fish
aozora-flavored-markdown completions fish > ~/.config/fish/completions/aozora-flavored-markdown.fish
```
