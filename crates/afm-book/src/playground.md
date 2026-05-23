# Try It Online

The fastest way to see Aozora Flavored Markdown render — without
installing Rust, cloning the repo, or even leaving the browser — is the
playground:

**<https://p4suta.github.io/afm/playground/>**

The page mounts a two-pane editor (CodeMirror 6 on the left, live HTML
on the right). The renderer is `afm-wasm` compiled to WebAssembly and
loaded in the page; everything happens client-side, so what you paste
never leaves your machine.

## What you can do

- **Edit afm source in CodeMirror.** Aozora notation gets a small
  syntax overlay highlighting `｜`, `《`, `》`, `［＃` and `※`.
- **Toggle 縦書き / 横書き.** The default is vertical; switching
  swaps the active stylesheet without re-rendering the wasm output,
  so it's instantaneous.
- **Load examples.** The dropdown ships seven curated snippets:
  welcome, ルビ catalogue, 傍点 catalogue, 縦中横, 改ページ／字下げ,
  paired containers, GFM × Aozora mixed.
- **Share a link.** *共有リンクをコピー* encodes the current source
  into the URL fragment via `lz-string`; the recipient opens an
  identical playground state by clicking the link.
- **Inspect diagnostics.** The bottom drawer surfaces every warning
  / error the parser emits (for example, an unclosed `［＃`) using
  the same wire-format `Diagnostic` shape that the library API
  exposes — so what you see in the playground matches what your
  Rust program would receive.

## How it's built

| Layer | Tech |
|---|---|
| Wasm | `crates/afm-wasm` via `wasm-pack --target bundler` |
| UI | Solid 1.9 components (Toolbar / EditorPane / PreviewPane / DiagnosticsDrawer) |
| Editor | CodeMirror 6 + `@codemirror/lang-markdown` + a custom aozora overlay |
| Toolchain | bun 1.3 + Vite 6 + `vite-plugin-wasm` + `vite-plugin-top-level-await` |
| Theme | The same `afm-vertical.css` / `afm-horizontal.css` the rest of this book uses |

The playground source lives under [`playground/`](https://github.com/P4suta/afm/tree/main/playground)
in the main repo. The `Justfile` recipes that drive it:

```text
just wasm-build           # release wasm pkg consumed by the playground
just wasm-build-dev       # fast dev-profile wasm for inner-loop iteration
just playground-install   # bun install inside the dev container
just playground-dev       # Vite dev server at http://localhost:5173/
just playground-dev-fast  # dev wasm + Vite dev server in one shot
just playground-build     # production build → playground/dist/
just playground-serve     # preview the production build locally
```

Every recipe runs inside the dev Docker container; no host-side bun
or Node install is needed (ADR-0002).

## When to use it

- **Demoing afm** to someone who doesn't have a Rust toolchain.
- **Quickly checking** whether a particular `［＃...］` annotation
  variant is recognised before reaching for the CLI.
- **On a phone or tablet** — the layout stacks vertically below
  720 px so the editor stays usable on narrow screens.

For library / CLI integration use cases, see
[Library Usage](library.md) and the [CLI Quickstart](cli.md).
