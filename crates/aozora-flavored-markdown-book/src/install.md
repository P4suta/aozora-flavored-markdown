# Install

aozora-flavored-markdown ships as a single `aozora-flavored-markdown` binary and as a Rust library. The two
entry points share the same parser core, so a CLI run and a library
embed produce identical HTML for the same input.

## From GitHub Releases

Pre-built binaries for the following targets are published to
[GitHub Releases](https://github.com/P4suta/aozora-flavored-markdown/releases):

| Target                            | Archive    |
|-----------------------------------|------------|
| `x86_64-unknown-linux-gnu`        | `.tar.gz`  |
| `x86_64-unknown-linux-musl`       | `.tar.gz`  |
| `aarch64-apple-darwin`            | `.tar.gz`  |
| `x86_64-apple-darwin`             | `.tar.gz`  |
| `x86_64-pc-windows-msvc`          | `.zip`     |

Each archive bundles the `aozora-flavored-markdown` binary alongside `LICENSE-MIT`,
`LICENSE-APACHE`, `README.md`, `CHANGELOG.md`, the shell completions
(`completions/`), and the man page (`man/aozora-flavored-markdown.1`). A release-wide
`SHA256SUMS` file is attached to the release for bulk verification:

```sh
# Replace vX.Y.Z with the release tag you want from the Releases page.
curl -L https://github.com/P4suta/aozora-flavored-markdown/releases/download/vX.Y.Z/SHA256SUMS -o SHA256SUMS
sha256sum --check --ignore-missing SHA256SUMS
tar xzf aozora-flavored-markdown-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz
aozora-flavored-markdown-vX.Y.Z-x86_64-unknown-linux-gnu/aozora-flavored-markdown --version
```

### Shell completions and man page

The archive's `completions/` directory holds scripts for bash (`aozora-flavored-markdown.bash`),
zsh (`_afm`), fish (`aozora-flavored-markdown.fish`), powershell (`_afm.ps1`), and elvish
(`aozora-flavored-markdown.elv`); install the one for your shell where it looks for
completions. The man page is `man/aozora-flavored-markdown.1`:

```sh
# zsh: copy `_afm` to a directory on your $fpath
cp completions/_afm ~/.zfunc/_afm
# man page
sudo cp man/aozora-flavored-markdown.1 /usr/local/share/man/man1/aozora-flavored-markdown.1 && man aozora-flavored-markdown
```

You can also print a completion script on demand without the archive —
`aozora-flavored-markdown completions <shell>` (see the [CLI Reference](ref/cli.md)).

## From source

```sh
git clone https://github.com/P4suta/aozora-flavored-markdown
cd aozora-flavored-markdown
just build-release
```

This produces `target/release/aozora-flavored-markdown`. The build runs inside the dev
Docker image per [ADR-0002](arch/adr.md); the host does not need a
Rust toolchain installed.

## As a Rust library

aozora-flavored-markdown is not on crates.io yet; depend on it directly by git URL:

```toml
[dependencies]
aozora-flavored-markdown = { git = "https://github.com/P4suta/aozora-flavored-markdown" }
```

See [Library Usage](library.md) for a minimal parse + render example.
