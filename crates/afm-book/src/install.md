# Install

afm ships as a single `afm` binary and as a Rust library. The two
entry points share the same parser core, so a CLI run and a library
embed produce identical HTML for the same input.

## From GitHub Releases

Pre-built binaries for the following targets are published to
[GitHub Releases](https://github.com/P4suta/afm/releases):

| Target                            | Archive    |
|-----------------------------------|------------|
| `x86_64-unknown-linux-gnu`        | `.tar.gz`  |
| `x86_64-unknown-linux-musl`       | `.tar.gz`  |
| `aarch64-apple-darwin`            | `.tar.gz`  |
| `x86_64-apple-darwin`             | `.tar.gz`  |
| `x86_64-pc-windows-msvc`          | `.zip`     |

Each archive bundles the `afm` binary alongside `LICENSE-MIT`,
`LICENSE-APACHE`, `NOTICE`, and `README.md`. A release-wide
`SHA256SUMS` file is attached to the release for bulk verification:

```sh
# Replace vX.Y.Z with the release tag you want from the Releases page.
curl -L https://github.com/P4suta/afm/releases/download/vX.Y.Z/SHA256SUMS -o SHA256SUMS
sha256sum --check --ignore-missing SHA256SUMS
tar xzf afm-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz
afm-vX.Y.Z-x86_64-unknown-linux-gnu/afm --version
```

## From source

```sh
git clone https://github.com/P4suta/afm
cd afm
just build-release
```

This produces `target/release/afm`. The build runs inside the dev
Docker image per [ADR-0002](arch/adr.md); the host does not need a
Rust toolchain installed.

## As a Rust library

afm is not on crates.io yet; depend on it directly by git URL:

```toml
[dependencies]
afm-markdown = { git = "https://github.com/P4suta/afm" }
```

See [Library Usage](library.md) for a minimal parse + render example.
