# Aozora Flavored Markdown

[English](./README.md) · [日本語](./README.ja.md)

<p align="center">
  <a href="https://github.com/P4suta/aozora-flavored-markdown/actions/workflows/ci.yml"><img alt="ci" src="https://github.com/P4suta/aozora-flavored-markdown/actions/workflows/ci.yml/badge.svg"></a>
  <a href="https://github.com/P4suta/aozora-flavored-markdown/actions/workflows/docs.yml"><img alt="docs deploy" src="https://github.com/P4suta/aozora-flavored-markdown/actions/workflows/docs.yml/badge.svg"></a>
  <a href="https://crates.io/crates/aozora-flavored-markdown-cli"><img alt="crates.io" src="https://img.shields.io/crates/v/aozora-flavored-markdown-cli?label=aozora-flavored-markdown-cli"></a>
  <a href="https://docs.rs/aozora-flavored-markdown"><img alt="docs.rs" src="https://img.shields.io/docsrs/aozora-flavored-markdown?label=docs.rs"></a>
  <a href="https://github.com/P4suta/aozora-flavored-markdown/releases/latest"><img alt="latest release" src="https://img.shields.io/github/v/release/P4suta/aozora-flavored-markdown?display_name=tag&sort=semver"></a>
  <a href="./LICENSE-APACHE"><img alt="license" src="https://img.shields.io/badge/license-Apache--2.0%20OR%20MIT-blue"></a>
  <a href="./rust-toolchain.toml"><img alt="msrv" src="https://img.shields.io/badge/rust-1.96%2B-orange"></a>
  <a href="https://codespaces.new/P4suta/aozora-flavored-markdown"><img alt="open in github codespaces" src="https://github.com/codespaces/badge.svg" height="20"></a>
</p>

<p align="center">
  📖 <a href="https://p4suta.github.io/aozora-flavored-markdown/"><strong>ドキュメント</strong></a>
  · 🧪 <a href="https://p4suta.github.io/aozora-flavored-markdown/api/"><strong>API リファレンス (rustdoc)</strong></a>
  · 📦 <a href="https://github.com/P4suta/aozora-flavored-markdown/releases"><strong>リリース &amp; バイナリ</strong></a>
  · 📝 <a href="./CHANGELOG.md"><strong>Changelog</strong></a>
</p>

**Aozora Flavored Markdown** は、[GitHub Flavored Markdown (GFM)](https://github.github.com/gfm/)
と同じ系譜の Markdown 方言です。青空文庫(Aozora Bunko)が長年にわたって
整備してきた日本語組版用の記法 —— ルビ、傍点、縦中横、`［＃…］` 注記、
外字、アクセント分解など —— を CommonMark + GFM の上に重ね、日本語の
縦書き・横書きどちらでも使える Markdown を提供します。

GFM と同様、aozora-flavored-markdown は CommonMark + GFM の **strict superset** です。
純粋な CommonMark / GFM 文書はそのまま同じ HTML にレンダリングされ、
青空文庫記法の拡張は入力が実際にそれを使った箇所でのみ発動します。
拡張子は `.md` のまま。単一の Rust クレート群と単一の `aozora-flavored-markdown`
バイナリが、通常の CommonMark パーサを使う場面にそのまま差し込めます。

本リポジトリは aozora-flavored-markdown の **仕様** ([`crates/aozora-flavored-markdown-book/`](./crates/aozora-flavored-markdown-book/)
配下の mdbook サイト)と **参照実装** の両方をホストしています —— GFM
が採用している分離方式と同じです。

## 系譜

Markdown 方言は、前の方言を拡張する形で積み重なってきました。aozora-flavored-markdown は
その上に日本語組版の層を載せています。

```
CommonMark  ──▶  GFM  ──▶  Aozora Flavored Markdown
(構造的な          (表、タスクリスト、  (ルビ、傍点、縦中横、字下げ、
 Markdown)          ~打ち消し線~)        外字、返り点、割注、
                                         アクセント分解、…)
```

青空文庫のボランティアが 20 年以上にわたって整備してきた
[注記記法](https://www.aozora.gr.jp/annotation/) を丸ごと取り込み、
現代の Markdown AST の上に写し取ります。結果として生成される HTML
は、CommonMark を受理できるどんなレンダリング pipeline にもそのまま
組み込めます。

## 強い保証

- **100% CommonMark / GFM 互換** —— 両 spec の conformance test suite を
  verbatim で全通過(CommonMark 0.31.2 の 652 ケース + GFM 0.29)。
- **100% 青空文庫記法互換をターゲット** —— <https://www.aozora.gr.jp/annotation/>
  が列挙するあらゆる記法を parse し、aozora-flavored-markdown は Tier-A invariant(HTML
  内に未消費の `［＃` を漏らさない)を保証します。
- **単一バイナリ**、実行時の外部プロセス依存なし。
- **Pure-functional な parse pipeline** —— vendored comrak 内に
  parse-time hook は 0、青空文庫記法の認識は sibling repo
  [`aozora`](https://github.com/P4suta/aozora) に分離され、
  `aozora-flavored-markdown::ast_splice` で comrak AST に splice されます。

## 書ける記法

```markdown
# 第一章                             (Markdown 見出し)
［＃「第一篇」は大見出し］            (青空文庫見出し、同じ AST へ合流)

彼は｜青梅《おうめ》に行った。        (ルビ)
それは《《強調したい》》ことだった。    (傍点)
令和［＃縦中横］2［＃縦中横終わり］年。 (縦中横)

［＃ここから字下げ］                  (ブロック字下げ)
段落……
［＃ここで字下げ終わり］
```

## ワークスペース構成

```
aozora-flavored-markdown/
  upstream/comrak/           # vendored comrak 0.52.0、verbatim (0 行 diff)
  crates/
    aozora-flavored-markdown/            # CommonMark + GFM + 青空文庫記法の HTML 統合レイヤ
    aozora-flavored-markdown-cli/                 # `aozora-flavored-markdown` バイナリ(render / check)
    aozora-flavored-markdown-book/                # mdbook ドキュメントサイト(Rust crate ではない)
    xtask/                   # upstream-sync / spec-refresh / new-adr
  spec/                      # CommonMark / GFM fixture
  docs/adr/                  # Architecture Decision Records
```

青空文庫記法の lex / 借用 AST / per-node HTML / serialize は sibling
repo [`P4suta/aozora`](https://github.com/P4suta/aozora) の
`aozora-syntax` / `aozora-pipeline` / `aozora-render` /
`aozora-encoding` / `aozora-spec` / `aozora-proptest` から git
依存で引いています(ADR-0010)。`Cargo.toml` の
`[workspace.dependencies]` が依存設定の単一の真実です。

## Sibling リポジトリ

| Repo | 内容 |
|---|---|
| [`P4suta/aozora`](https://github.com/P4suta/aozora) | 純粋な青空文庫記法パーサ —— lexer / AST / renderer / 外字テーブル。 |
| [`P4suta/aozora-tools`](https://github.com/P4suta/aozora-tools) | 執筆支援ツール: `aozora-fmt` formatter / `aozora-lsp` Language Server / tree-sitter grammar / VS Code extension。 |

## 開発

すべての操作は Docker 内で動作します。ホストの toolchain は直接は
呼びません(ADR-0002)。

```bash
just test              # Docker 経由で cargo nextest
just lint              # fmt + clippy + typos + strict-code
just coverage          # llvm-cov regions、CI floor は 96%
just spec-commonmark   # CommonMark 0.31.2 spec フル
just spec-gfm          # GFM 0.29 spec
just upstream-diff     # 上流 comrak 比 0 行 diff の確認(verbatim v0.52.0)
just ci                # full CI matrix のローカル再現
just book-serve        # mdbook 即時プレビュー
```

青空文庫専用のテスト面(`spec-aozora` / `spec-golden-56656` /
`corpus-sweep`)は sibling repo
[`P4suta/aozora`](https://github.com/P4suta/aozora) に置かれています。
そちらから実行してください。

詳しくは [CLAUDE.md](./CLAUDE.md) (プロジェクトガイド)、
[docs/adr/](./docs/adr/) (Architecture Decisions)、
[CONTRIBUTING.md](./CONTRIBUTING.md) (貢献方法)を参照してください。

## サンプル

ライブラリ利用向けの短いサンプルが
[`crates/aozora-flavored-markdown/examples/`](./crates/aozora-flavored-markdown/examples/) に
あります。

- `render-utf8.rs` —— UTF-8 ファイルを parse して HTML を stdout へ。
- `render-sjis.rs` —— Shift_JIS の青空文庫テキストを `aozora-encoding`
  経由で decode してから HTML に。
- `ast-walk.rs` —— parse した AST を walk して AozoraNode 種別を集計。
- `serialize-round-trip.rs` —— `serialize ∘ parse ≡ id` を 1 ファイルで
  確認。

実行:

```sh
cargo run --example <name> -p aozora-flavored-markdown -- <path/to/input.md>
```

## インストール

### CLI

crates.io から:

```sh
cargo install aozora-flavored-markdown-cli
```

**Linux x86_64**, **macOS arm64**, **Windows x86_64** 用のビルド済み
バイナリが各 GitHub Release に添付されています ——
[releases ページ](https://github.com/P4suta/aozora-flavored-markdown/releases) から
`aozora-flavored-markdown-vX.Y.Z-<target>.{tar.gz,zip}` を選んでください。SHA256 sum は
`SHA256SUMS` として併置されます。各アーカイブにはバイナリ・シェル補完・
`aozora-flavored-markdown.1` man ページが同梱されます。

最新の開発版を git から:

```sh
cargo install --git https://github.com/P4suta/aozora-flavored-markdown --locked aozora-flavored-markdown-cli
```

### ライブラリ

```sh
cargo add aozora-flavored-markdown
```

```rust
use aozora_flavored_markdown::{Options, render_to_string};

let rendered = render_to_string("彼は｜青梅《おうめ》に行った。", &Options::default());
assert!(rendered.html.contains("<ruby>"));
```

完全な API は [docs.rs](https://docs.rs/aozora-flavored-markdown) にあります。
出力 HTML は安定した `aozora-md-*` CSS クラスを持ちます（[ADR-0011](docs/adr/0011-brand-boundary-css-class-rewrite.md)）。

## セキュリティ

脆弱性は GitHub Security Advisories 経由で報告してください ——
開示フローは [`SECURITY.md`](./SECURITY.md) を参照。

## ライセンス

Rust コミュニティ慣例にしたがい、[Apache-2.0](./LICENSE-APACHE) OR
[MIT](./LICENSE-MIT) のデュアルライセンスです。

vendored `upstream/comrak/` は上流のライセンス(BSD-2-Clause)のまま
です。`upstream/comrak/COPYING` を参照。

パーサ側の spec / golden / corpus fixture が利用する青空文庫の
本文は sibling repo [`P4suta/aozora`](https://github.com/P4suta/aozora)
側にまとめてあります(public domain、作品ごとに作者・訳者などの帰属を
記載)。aozora-flavored-markdown 本体は `spec/` 配下に CommonMark 0.31.2 / GFM 0.29 の
spec fixture のみを同梱しています。

第三者由来素材の帰属は [NOTICE](./NOTICE) に集約しています
(vendored comrak、CommonMark / GFM spec fixture、青空文庫の仕様
および作品)。
