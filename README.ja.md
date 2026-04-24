# Aozora Flavored Markdown (afm)

[English](./README.md) · [日本語](./README.ja.md)

<p align="center">
  <a href="https://github.com/P4suta/afm/actions/workflows/ci.yml"><img alt="ci" src="https://github.com/P4suta/afm/actions/workflows/ci.yml/badge.svg"></a>
  <a href="./LICENSE-APACHE"><img alt="license" src="https://img.shields.io/badge/license-Apache--2.0%20OR%20MIT-blue"></a>
  <a href="./rust-toolchain.toml"><img alt="msrv" src="https://img.shields.io/badge/rust-1.95%2B-orange"></a>
  <a href="https://p4suta.github.io/afm/"><img alt="docs" src="https://img.shields.io/badge/docs-GitHub%20Pages-blue"></a>
  <a href="https://codespaces.new/P4suta/afm"><img alt="open in github codespaces" src="https://github.com/codespaces/badge.svg" height="20"></a>
</p>

**Aozora Flavored Markdown** (afm) は、[GitHub Flavored Markdown (GFM)](https://github.github.com/gfm/)
と同じ系譜の Markdown 方言です。青空文庫(Aozora Bunko)が長年にわたって
整備してきた日本語組版用の記法 —— ルビ、傍点、縦中横、`［＃…］` 注記、
外字、アクセント分解など —— を CommonMark + GFM の上に重ね、日本語の
縦書き・横書きどちらでも使える Markdown を提供します。

GFM と同様、afm は CommonMark + GFM の **strict superset** です。
純粋な CommonMark / GFM 文書はそのまま同じ HTML にレンダリングされ、
青空文庫記法の拡張は入力が実際にそれを使った箇所でのみ発動します。
拡張子は `.md` のまま。単一の Rust クレート群と単一の `afm`
バイナリが、通常の CommonMark パーサを使う場面にそのまま差し込めます。

本リポジトリは afm の **仕様** ([`crates/afm-book/`](./crates/afm-book/)
配下の mdbook サイト)と **参照実装** の両方をホストしています —— GFM
が採用している分離方式と同じです。

**ステータス**: v0.1.0 public preview —— 中核となる記法は機能的に
完備、SemVer 0.x 契約で API の破壊的変更の余地は残しつつ、日常利用を
歓迎します。

## 系譜

Markdown 方言は、前の方言を拡張する形で積み重なってきました。afm は
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
  が列挙するあらゆる記法を parse し、『罪と罰』(青空文庫カード 56656)
  の fixture が Tier-A acceptance gate を通過(panic なし、HTML 内に
  未消費の `［＃` なし)。
- **17 k 作品コーパスでの invariant sweep** —— CI で 4 つの invariant
  を enforce: I1 panic しない / I2 bare `［＃` を漏らさない /
  I3 `serialize ∘ parse` が fixed point / I4 生成した HTML が
  タグ balanced。`crates/afm-parser/tests/corpus_sweep.rs` と ADR-0007
  を参照。
- **単一バイナリ**、実行時の外部プロセス依存なし。
- **Pure-functional な parse pipeline** (ADR-0008) —— vendored comrak
  内に parse-time hook は 0、Aozora 記法の認識は `afm-lexer` と
  `afm-parser` の post-process AST splice に完全に分離。

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
afm/
  upstream/comrak/           # vendored comrak 0.52.0、ADR-0001 の 200 行 diff 予算
  crates/
    afm-syntax/              # AozoraNode AST 型(パーサ依存なし)
    afm-lexer/               # 7-phase pure-functional な青空文庫 lexer (ADR-0008)
    afm-parser/              # post_process AST splice + HTML renderer
    afm-encoding/            # Shift_JIS decode + 青空文庫外字解決
    afm-cli/                 # `afm` バイナリ
    afm-corpus/              # 17 k 作品コーパス回帰 harness
    afm-book/                # mdbook ドキュメントサイト(Rust crate ではない)
    xtask/                   # upstream-sync / upstream-diff / spec-refresh / new-adr
  spec/                      # CommonMark / GFM / 青空文庫 fixture
  docs/adr/                  # Architecture Decision Records
```

## 開発

すべての操作は Docker 内で動作します。ホストの toolchain は直接は
呼びません (ADR-0002)。

```bash
just test              # Docker 経由で cargo nextest
just lint              # fmt + clippy + typos + strict-code
just coverage          # llvm-cov regions、CI floor は 96%
just spec-commonmark   # CommonMark 0.31.2 spec フル
just spec-gfm          # GFM 0.29 spec
just spec-aozora       # 手書き青空文庫記法 fixture
just spec-golden-56656 # 『罪と罰』 Tier-A acceptance gate
just corpus-sweep      # 17 k 作品 invariant sweep (I1–I4)
just upstream-diff     # 上流 comrak 比 200 行予算 check
just ci                # full CI matrix のローカル再現
just book-serve        # mdbook 即時プレビュー
```

**At a glance**: 519 tests passing · 96% regions coverage (CI gate) ·
上流 comrak 比 ~22 行 diff · vendored comrak 内に parse-time hook 0 本。

詳しくは [CLAUDE.md](./CLAUDE.md) (プロジェクトガイド)、
[docs/adr/](./docs/adr/) (Architecture Decisions)、
[CONTRIBUTING.md](./CONTRIBUTING.md) (貢献方法)を参照してください。

## サンプル

ライブラリ利用向けの短いサンプルが
[`crates/afm-parser/examples/`](./crates/afm-parser/examples/) に
あります。

- `render-utf8.rs` —— UTF-8 ファイルを parse して HTML を stdout へ。
- `render-sjis.rs` —— Shift_JIS の青空文庫テキストを `afm-encoding`
  経由で decode してから HTML に。
- `ast-walk.rs` —— parse した AST を walk して AozoraNode 種別を集計。
- `serialize-round-trip.rs` —— `serialize ∘ parse ≡ id` を 1 ファイルで
  確認。

実行:

```sh
cargo run --example <name> -p afm-parser -- <path/to/input.md>
```

## ステータス

**v0.1.0 public preview**。ドキュメント化された機能について API は
安定していますが、プロジェクトは SemVer 0.x 段階です —— 1.0 前に
API 破壊的変更が発生する可能性があります。
リリース履歴は [CHANGELOG.md](./CHANGELOG.md) を参照。

## ライセンス

Rust コミュニティ慣例にしたがい、[Apache-2.0](./LICENSE-APACHE) OR
[MIT](./LICENSE-MIT) のデュアルライセンスです。

vendored `upstream/comrak/` は上流のライセンス(BSD-2-Clause)のまま
です。`upstream/comrak/COPYING` を参照。

`spec/aozora/fixtures/` 配下の本文は青空文庫(public domain)由来で、
作品ごとに作者・訳者などの帰属を記載しています。

第三者由来素材の帰属は [NOTICE](./NOTICE) に集約しています
(vendored comrak、CommonMark / GFM spec fixture、青空文庫の仕様
および作品)。
