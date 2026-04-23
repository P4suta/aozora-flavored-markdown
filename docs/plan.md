# aozora-flavored-markdown (afm) 実装計画

## Context

日本語で文章を書く現代の書き手は、Markdown (構造は得意だがルビ・傍点・縦中横を扱えない) と青空文庫記法 (組版は完璧だが表・リンク・コードに弱い) の間で選択を強いられている。Re:VIEW 等の重量級ツールは学習コストが高い。**afm** はこの空白を埋める「青空文庫風味の Markdown」であり、Rust 製シングルバイナリ・単独で動く。

成果物は (1) 記法仕様、(2) CommonMark/GFM 100% 互換かつ青空文庫 100% 互換のパーサ、(3) 縦書きCSSを同梱した HTML バックエンド。MVP のワンパスは「afm テキスト → 縦書き/横書き HTML」。

開発方法論の柱は **Docker 完全封じ込め + TDD + C1 100% branch coverage + Production-level error handling + 最新版追従 + 業界標準ツール積極採用**。全操作を `docker compose run` + `Justfile` 経由に統一し、「ローカルで動いた/CIで落ちた」の揺れを構造的にゼロにする。エラー発生時は推測で直さず**公式ドキュメントを第一情報源**として調査した上で修正する。

## 確定した設計判断

### 採用する基盤と記法

| 項目 | 判断 | 根拠 |
|---|---|---|
| 言語 / MSRV | Rust 1.95.0 (2026-04 時点 stable、MSRV 固定) | 性能・単一バイナリ・CJK 処理ライブラリ充実 |
| Markdown パーサ基盤 | **comrak 0.52.0 を fork (vendor-in-tree)** | pulldown-cmark の event stream ではルビ等の intra-token parse 不可。markdown-rs は標準拡張のみ受付 |
| ルビ記法 | **案A 単独** `｜漢字《かんじ》` / `漢字《かんじ》` | 青空文庫 100% 互換要件上唯一。案C (`[漢字]{かんじ}`) は明示的に非サポート |
| ブロック組版記法 | **案X 単独** `［＃ここから字下げ］...［＃ここで字下げ終わり］` | 青空文庫 .txt 直接食わせ互換のため |
| 入力エンコーディング | parser は UTF-8 純粋、SJIS 変換は `afm-encoding` + CLI | parser をライブラリとして純粋に保ちつつ、青空文庫特有のエンコーディング事情は別クレートで完結 |
| 100% 互換の定義 | **全青空文庫記法を semantic HTML 化**。pass-through は不可 | ユーザー決定。M2 のスコープは 2週 → 4-5週 に拡大 |
| ファイル拡張子 | `.md` (独自拡張子不使用) | Markdown 方言としてのポジショニング。素の MD パーサでも大部分が動く後方互換性 |

### 主要依存クレート (全て 2026-04 現在の最新 stable)

ランタイム: `comrak 0.52.0` / `winnow 1.0.2` / `clap 4.6.1` / `miette 7.6.0` / `thiserror 2.0.18` / `anyhow 1.0.102` (CLI 最終層) / `encoding_rs 0.8.35` (SJIS) / `regex 1.12.3` / `unicode-segmentation 1.10` / `serde 1.0.228` / `serde_json 1.0.149` / `tracing 0.1` / `tracing-subscriber 0.3`

テスト: `insta 1.38+` / `proptest 1.x` (property-based) / `criterion 0.5+` (benchmark) / `cargo-fuzz` (fuzzing, libFuzzer 統合)

Dev tooling: `cargo-nextest 0.9.133+` / `cargo-llvm-cov 0.8.5` (C1 branch coverage) / `cargo-deny 0.19.4` / `cargo-audit 0.22.1` / `cargo-udeps` / `cargo-semver-checks` / `typos` / `lefthook` / `commitlint` / `release-please` / `mdbook 0.4.40` / `mdbook-linkcheck`

Edition: **Rust 2024 edition** (1.85+ で利用可、1.95 前提なので問題なし)。古い 2021 edition での新規モジュール作成は不可。

バージョンは Dependabot による自動追従、GitHub Actions は SHA pin + Dependabot でバンプ (cron/schedule は入れない、Dependabot weekly は例外許容)。deprecated な依存/API を見つけ次第モダンな代替へ置換する。

## アーキテクチャ

### クレート構成 (5 crates)

```
afm/
  Cargo.toml                    # virtual workspace
  rust-toolchain.toml           # channel = "1.95.0"
  upstream/comrak/              # comrak 0.52.0 verbatim (git-tracked)
    COMRAK_SHA                  # pinned upstream sha
    UPSTREAM_DIFF.md            # auto-generated
  crates/
    afm-syntax/                 # AozoraNode enum + span/error 型 (no comrak dep)
    afm-parser/                 # forked comrak + aozora 拡張 + html renderer (feature)
    afm-encoding/               # Shift_JIS 変換 + 青空文庫 gaiji 正規化 + BOM 判定
    afm-cli/                    # clap binary 'afm'
    afm-book/                   # mdbook docs (push-triggered, not cron)
    xtask/                      # upstream-sync, upstream-diff, corpus-refresh, bench
  spec/
    commonmark-0.31.2.json      # vendored fixtures
    gfm-0.29-gfm.json
    aozora/                     # hand-written annotation fixtures + corpus.lock
```

**afm-html は afm-parser 内の module として統合** (feature `html`, default on)。comrak が parser+render を同一クレートで扱う設計に追随しフォーク境界を単純化する。代わりに **afm-encoding を独立クレート化**: SJIS 処理と青空文庫 gaiji マップをライブラリとして再利用可能にするため。

### comrak フォーク運用

**vendor-in-tree 一択。** submodule は crates.io 公開時に破綻、subtree は履歴汚染、patch-quilt は rust-analyzer 非互換。

**diff 最小化戦略**: upstream ファイルの編集は「1 行 hook 呼び出し追加」のみ。拡張本体は `crates/afm-parser/src/aozora/` 配下。

| upstream ファイル | 追加行 |
|---|---|
| `upstream/comrak/src/nodes.rs` | `NodeValue::Aozora(afm_syntax::AozoraNode)` 1 variant |
| `upstream/comrak/src/parser/inline.rs` | `｜` `《` 文字 dispatch に `aozora::inline::try_parse(...)` |
| `upstream/comrak/src/parser/block.rs` | block-start loop に `aozora::block::try_start(...)` |
| `upstream/comrak/src/html.rs` | `NodeValue::Aozora` arm で `aozora::html::render(...)` |
| `upstream/comrak/src/lib.rs` | `ExtensionOptions.aozora: bool` flag |

**ハード制約**: `xtask upstream-diff` で `git diff upstream/v0.52.0 -- upstream/comrak/` の行数を計測、**200 行超で CI FAIL**。これが守られる限り upstream の quarterly マージは three-way merge で済む。

**upstream-sync 運用**: 四半期ごと or リリース追従。`xtask upstream-sync <tag>` が新 tag をチェックアウトし、hook 追加パッチ (`hooks.patch`) を再適用、全テストスイート実行。

## AST (afm-syntax)

設計書 §4.4 のスケッチに **探索で判明した欠落要素を追加**:

```rust
pub enum AozoraNode {
    // インライン
    Ruby { base: String, reading: String, delim_explicit: bool },
    Bouten { kind: BoutenKind, children: Vec<NodeId> },
    // BoutenKind: Goma (・), Janome (◉), DoubleCircle, Circle, WhiteCircle, WavyLine, UnderLine
    TateChuYoko(String),
    Gaiji { ucs: Option<char>, mencode: Option<String>, desc: String },

    // ブロック (paired)
    Indent { amount: u8, children: Vec<NodeId> },        // ［＃ここから字下げ］
    AlignEnd { offset: u8, children: Vec<NodeId> },       // 地付き / 地から N 字上げ
    Warichu(Vec<NodeId>),                                 // 割り注
    Keigakomi(Vec<NodeId>),                               // 罫囲み

    // ブロック (leaf)
    PageBreak,                                             // ［＃改ページ］
    SectionBreak(SectionKind),                             // 改丁/改段/改見開き
    AozoraHeading { level: HeadingLevel, children: Vec<NodeId> },
    // HeadingLevel: Large/Medium/Small/Sub/Window + ［＃大見出し］→ Markdown `#` へ alias
    Sashie { file: String, caption: Option<Vec<NodeId>> }, // ［＃挿絵（fig.png）入る］

    // 保険
    Annotation { raw: String, kind: AnnotationPassthrough }, // 未知注記の raw 保持 (警告付き)
}
```

**アーキテクチャ上の鍵**: comrak 側 `NodeValue` は 1 variant (`Aozora(AozoraNode)`) しか増やさない。拡張 AST は全て `afm-syntax::AozoraNode` 内に閉じ込める。upstream diff が最小化されリファクタに強い。

Markdown 構造と青空文庫のエイリアスは**パース時に正規化**する:
- `［＃大見出し］...［＃大見出し終わり］` → `NodeValue::Heading { level: 1 }` (通常の H1 に吸収)
- `［＃挿絵（fig.png）入る］` → `NodeValue::Image` (alt="")

これにより「同じ概念・別記法」で AST が分岐せず、HTML 出力も統一される。

## パーサ戦略

### ルビのインライン介入

`src/parser/inline.rs` の文字 dispatch に 2 分岐を追加:

- `｜ (U+FF5C)`: 前方スキャン → kanji/numeric 連続取得 → `《` 必須 → `》` まで consume。明示デリミタ (`delim_explicit: true`)。
- `《 (U+300A)`: 直前 text token を rewind、kanji 連続を base として切出し。暗黙デリミタ。kanji 境界判定は `unicode-segmentation` + CJK ブロック判定。

**タイミング**: 強調の post-process より**前**、リンクのブラケット push より**後**。これにより `[漢字《かんじ》](url)` は「リンクの中にルビ」として自然に成立する。逆方向 `｜[漢字](url)《かんじ》` は spec 上未定義 → `Annotation { kind: InvalidRubySpan }` として diagnostics 付きで passthrough。

**強調ネスト**: `*漢字《かんじ》*` は問題なし (ルビ node は delimiter run ではないため emph post-process に触れない)。

### ブロック注記のパース

`［＃...］` は**新規 block starter として直接挿入** (pre-pass 方式は span 情報喪失により miette 診断が劣化)。

`crates/afm-parser/src/aozora/block.rs`:

```rust
pub(crate) fn try_start(parser: &mut Parser, container: &mut AstNode) -> StartResult {
    // ［＃ で始まる行頭マッチ (indent ストリップ後)
    // ディスパッチ → 字下げ / 地付き / 割り注 / 罫囲み / 改ページ / 見出し / 挿絵 / 縦中横 / gaiji / その他
    // paired: parser.aozora_stack に push し container 化
    // leaf: 1-line 消費して AozoraNode 生成
}
```

paired 注記 (字下げ, 地付き, 割り注, 罫囲み, 縦中横) の状態は `parser.aozora_stack: Vec<AozoraOpen>` で管理。comrak の未閉 HTML ブロック処理と同じパターン。

### エイリアス正規化

`［＃大見出し］...［＃大見出し終わり］` のような Markdown 構造に写像可能な注記は **parse 段階で標準 `NodeValue::Heading` に変換**。`AozoraHeading` は窓見出し等の青空文庫固有レベルだけが使う。

## HTML レンダリング (全記法 semantic)

設計書 §5.2 に加え、ユーザー決定「全記法 semantic HTML 化」に従う:

| AozoraNode | HTML |
|---|---|
| `Ruby` | `<ruby>base<rp>(</rp><rt>reading</rt><rp>)</rp></ruby>` |
| `Bouten(Goma)` | `<em class="afm-bouten afm-bouten-goma">...</em>` |
| `TateChuYoko` | `<span class="afm-tcy">...</span>` (CSS: `text-combine-upright: all`) |
| `Gaiji` | `<span class="afm-gaiji" title="desc">ucs もしくは desc</span>` |
| `Indent{amount}` | `<div class="afm-indent afm-indent-{amount}">...</div>` |
| `AlignEnd{offset}` | `<div class="afm-align-end" style="--offset: {offset}">...</div>` |
| `Warichu` | `<span class="afm-warichu"><span class="afm-warichu-upper">...</span><span class="afm-warichu-lower">...</span></span>` |
| `Keigakomi` | `<div class="afm-keigakomi">...</div>` (CSS: border) |
| `PageBreak` | `<div class="afm-page-break" aria-hidden="true"></div>` (CSS: `page-break-before: always; break-before: page`) |
| `SectionBreak` | `<div class="afm-section-break afm-section-break-{kind}"></div>` |
| `Sashie` | `<figure class="afm-sashie"><img src="..." alt=""><figcaption>...</figcaption></figure>` |

**付属 CSS** (別ファイルで配布、本リポジトリ成果物): `afm-vertical.css` (`writing-mode: vertical-rl`) / `afm-horizontal.css`。minimal テンプレートとして提供。

**割り注の semantic 化** は青空文庫の意味論 (上下 2 行の注釈) を保存するため上下分割構造で出力。CSS で 2-line 縦積み表示。

## Gaiji 処理 (afm-encoding)

青空文庫の `※［＃「component」、JIS-level-area-point］` / `※［＃「component」、U+XXXX、page-line］` を解決:

1. 注記をパース (`afm-encoding::gaiji::parse`) → 記述文字列 + JIS/UCS コード取得。
2. ルックアップ: 内蔵 `jisx0213-gaiji.csv` (青空文庫配布のマップを vendor) で JIS X 0213 plane 3/4 → UCS 解決。
3. 出力: UCS 文字があればそれ、なければ記述文字列 + `<span class="afm-gaiji-unresolved" title="...">` でグレースフルデグレード。

`afm-encoding` の公開 API は:

```rust
pub fn decode_sjis(input: &[u8]) -> Result<String, DecodeError>;
pub fn detect_encoding(input: &[u8]) -> Encoding;  // BOM + heuristic
pub mod gaiji {
    pub fn resolve(desc: &str, code: Option<GaijiCode>) -> GaijiResolution;
}
```

CLI はこれをそのまま使う。parser は gaiji を**文字列として受け取り**、ノード生成のみ担当 (encoding 依存ゼロ)。

## テスト戦略 / CI

### CI ジョブ (GitHub Actions, SHA-pinned)

| ジョブ | 内容 | 合格基準 |
|---|---|---|
| `test-commonmark` | vendored `commonmark-0.31.2.json` (652 cases) | 652/652 |
| `test-gfm` | `gfm-0.29-gfm.json` | 全合格 |
| `test-aozora-spec` | `spec/aozora/*.json` (手書き注記別 ~40) | 全合格 |
| `test-aozora-corpus` | 青空文庫 120 作品 (SHA256 pin) | 下記 Tier A + B |
| `test-unit` | `cargo nextest run --workspace` | 全合格 |
| `test-proptest` | `proptest` (round-trip, 冪等性, 境界値) | shrink 後も全合格 |
| `test-fuzz-smoke` | `cargo-fuzz run <harness> -max_total_time=60` | 各 harness panic-free |
| `bench-regression` | `criterion --save-baseline` + 前回比較 | baseline 比 > +20% で警告、> +50% で fail |
| `coverage` | `cargo llvm-cov --branch` | `aozora/**` 100% (C1)、`afm-syntax/afm-encoding` 100%、upstream comrak 除外 |
| `upstream-diff` | diff 行数計測 | ≤ 200 行 |
| `deny` | `cargo deny check` | 全 OK |
| `audit` | `cargo audit` | 脆弱性ゼロ |
| `udeps` | `cargo udeps` | 未使用依存ゼロ |
| `semver` | `cargo semver-checks` (公開後) | 破壊的変更はメジャーバンプ要 |
| `fmt` | `cargo fmt --check` | clean |
| `clippy` | `cargo clippy -- -D warnings -W clippy::pedantic -W clippy::nursery` | 警告抑制なし |
| `typos` | `typos` | clean |
| `commitlint` | Conventional Commits 準拠 | clean |
| `book` | `mdbook build` + `mdbook-linkcheck` | clean |
| `e2e-browser` (M3 以降) | Playwright on Chromium + WebKit | CSS 縦書きレンダリング確認 |

**警告抑制は一切しない** (ignore/suppress/continue-on-error は不使用、根本 fix)。

### 青空文庫コーパス仕様

- **ソース**: `aozorabunko/aozorabunko_text` GitHub mirror (plaintext, SJIS)
- **固定**: `spec/aozora/corpus.lock` に 120 作品 × (card-id, title, author, SHA256) を記録。`xtask corpus-refresh` で更新。
- **フォールバック**: `spec/aozora/corpus-snapshot.tar.zst` を vendor (public domain 作品のみ、取り下げリスク対策)。
- **Tier A (hard)**: parser panic ゼロ、全 `［＃` シーケンス消費済み。120/120。
- **Tier B (hard)**: round-trip 安定性 (`parse → serialize → parse` で AST 一致)。120/120。
- **Tier C (M3 以降で hard)**: 青空文庫公式 HTML とクラス名マッピングを除き diff-clean。

### ゴールデンフィクスチャ (ユーザー提供、M0 Spike 受入基準)

ユーザーから day-1 テスト素材として下記が提供済み:

- **入力**: ドストエフスキー『罪と罰』米川正夫訳 (card-id **56656**) — `tsumito_batsu.txt` (Shift_JIS, 1.35 MB)
- **ゴールデン HTML**: 青空文庫公式レンダリング (`56656_74440.html`, 1.5 MB)

この作品は下記 edge case を網羅する稀有な単一素材:
- ルビ 1044 箇所 (明示デリミタ `｜`, 暗黙デリミタ両形)
- **前方参照傍点** `［＃「X」に傍点］` (ブラケット内で前方文字列を指定する特殊形式)
- **前方参照見出し** `［＃「第一篇」は大見出し］`
- **JIS X 0213 gaiji** `※［＃「木＋吶のつくり」、第3水準1-85-54］`
- **アクセント分解** `〔Crevez chiens, si vous n'e^tes pas contents〕`
- **ママ注記** `［＃「」」はママ］`
- **字下げ** `［＃２字下げ］`, `［＃７字下げ］`

**Day-1 受入パス**: リポジトリ初期化時点で `spec/aozora/fixtures/56656/` にこれら 2 ファイルを vendor (public domain) し、`just spec-golden-56656` で常時通す。M0 Spike 完了判定は下記:

- Tier A: parser panic ゼロ、未消費 `［＃` ゼロ
- Tier B: round-trip AST 一致
- Tier C: golden HTML 構造一致 (クラス名差異は許容、見出し階層・ルビ位置・傍点位置は厳密一致)

M1/M2 段階の回帰ベンチマークも本作品を baseline に使う (`criterion` で parse 時間を記録)。

### TDD + C1 100% coverage

- **TDD**: 失敗テストを先に書く (記法ノード追加時もスペック → パーサ → レンダラの順)。
- **C1 branch coverage 100%**: `aozora/**` / `afm-syntax` / `afm-encoding` に限定。upstream comrak は対象外 (追いかけるのは負け戦)。
- **Production-level errors**: `miette` で span 付き診断を全 parse error に実装。unknown 注記は `Annotation` node に格納して警告出力。

### スナップショットテスト

`insta` で代表入力に対する AST と HTML をスナップショット化。記法追加/挙動変更の影響可視化に使う。`cargo insta review` をローカル開発フローに組込み。

## 開発方法論 (Methodology)

本プロジェクトは以下 5 本柱を **Day 1 から妥協なく** 敷く:

### (1) Docker 封じ込め — 直接 toolchain を叩かない

**全操作を Docker コンテナ越しで実行する。** ローカルの Rust/Node/Python バージョン差による「動いた/動かない」の揺れをゼロに固定し、CI と開発機の環境を完全に同一化する。

- **禁止**: `cargo run` / `cargo test` / `cargo fmt` / `mdbook` / `playwright` 等をホスト直接実行
- **必須**: `make test` / `just build` / `./bin/afm` 等の thin wrapper 経由でのみコンテナ実行

具体構成:

```
/afm/
  docker-compose.yml          # サービス: dev, ci, book, browser
  Dockerfile                  # Rust 1.95.0 + Node 22 + 各種 cargo-xxx
  Justfile                    # 全操作の唯一のエントリポイント
  bin/
    afm                       # CLI を docker compose run afm-cli で包むスクリプト
    xtask                     # xtask を docker 経由で実行
  .devcontainer/
    devcontainer.json         # docker-compose.yml の dev サービスを利用
```

`docker-compose.yml` サービス:
- `dev`: 対話開発用 (VS Code Dev Containers の接続先)。mount: workspace、cargo registry volume、target volume
- `ci`: CI と同一の読み取り専用環境。ローカル再現用 (`just ci`)
- `book`: mdbook serve + linkcheck
- `browser`: Playwright + Chromium + WebKit (M3 以降)

`Justfile` が全ユースケースを網羅:
```
just test             # docker compose run --rm dev cargo nextest run
just lint             # fmt + clippy + typos すべて
just coverage         # llvm-cov --branch --fail-under-branches 100
just spec-commonmark  # CommonMark spec 全通し
just corpus           # 青空文庫 120 作品 Tier A+B
just upstream-diff    # 200 行バジェットチェック
just ci               # CI 相当の全ジョブをローカル再現
just book-serve       # mdbook ライブプレビュー
just e2e              # Playwright (M3 以降)
just upstream-sync <tag>  # comrak フォーク同期
```

**CI も同一 Docker イメージを使う** (`docker compose run ci just test` のパターン)。CI と手元で「動くが結果が違う」事態が構造的に発生しない。

rust-toolchain.toml の固定 + Dockerfile の多段ビルドキャッシュで pull 後の初期化も 1 分以内。

### (2) TDD + C1 分岐カバレッジ 100%

**失敗テストを先に書き、必要最小の実装で通す**。青空文庫記法は「仕様が緩く実コーパスで実態が決まる」性質を持つため、テスト無しで実装を積むと仕様ドリフトに気づけない。

- 記法ノード追加のフロー: spec fixture (`spec/aozora/<name>.json`) 追加 → `AozoraNode` variant 追加 → parser テスト (失敗) → parser 実装 → renderer テスト (失敗) → renderer 実装 → snapshot `cargo insta review`
- **C1 branch coverage 100%** を CI 必須ゲートとする: `just coverage` が `--fail-under-branches 100` で落ちる
- 対象: `afm-syntax/**`, `afm-encoding/**`, `afm-parser/src/aozora/**`, `afm-cli/src/**` (upstream comrak は対象外、追いかけるのは無理筋)
- 実現不可能と判明した分岐 (defensive な unreachable 等) は `#[cfg_attr(coverage, coverage(off))]` で明示除外、理由をコミットメッセージに残す — ignore/suppress は基本禁止
- 複数テストから異なる角度で不変条件を検証 (カバレッジは副産物、網羅が目的化しないように)

### (3) Production-level エラーハンドリング

**エラーは first-class citizen**。ユーザーだけでなく開発者自身の生産性を上げる投資として扱う。

- **`miette 7.6` で全 parse error に span 付き診断**。どの行・どの文字で何が壊れたかをカラー出力
- **`thiserror 2.0` で crate 境界のエラー型を区別**: `afm_syntax::Error`, `afm_parser::Error`, `afm_encoding::Error`, `afm_cli::Error`。`anyhow` は CLI の最終層のみ
- **未知注記の passthrough + warning**: `AozoraNode::Annotation { raw, kind: Unknown }` に格納、CLI `--strict` モードで fatal 化
- **エラーメッセージは日本語 first + 英語併記**: 青空文庫コミュニティに配慮
- **回復可能性の明示**: parser は partial AST + 診断リストで返し、CLI が判断
- **診断の例**:
  ```
  error[AFM0201]: ルビ開始デリミタ《 の対応する》 が見つからない
    ┌─ examples/kokoro.md:12:8
    │
  12│ 彼は｜青梅《おうめ に行った
    │        ^^^^^^ ここで始まったルビ
    │               ──── 》が必要
    │
    = help: 《 で開始したルビは必ず 》 で閉じる必要があります
  ```

**警告抑制は一切禁止**: `#[allow(...)]`, `continue-on-error`, `--ignore-unfixed` 等は全て禁止。根本 fix する (既存 feedback_no_warning_suppression に準拠)。

### (4) 最新版追従 + 公式ドキュメント第一主義

**バージョン方針**:
- 全依存は毎週 Dependabot で最新 stable に追従 (lockfile コミット、SHA pin + バンプ)
- Rust toolchain も `rust-toolchain.toml` を定期的に更新 (MSRV ladder は維持せず単一 pin)
- 古い API・deprecated な書き方を見つけたら即置き換え (技術的負債として残さない)

**モダン Rust の標準慣習を採用**:
- Edition 2024 (1.85+ で利用可) を使用
- `std::sync::OnceLock` / `LazyLock` など最新標準ライブラリ機能を優先 (外部 crate の `once_cell` より)
- `let-else`, `if let chains`, `let chains` 等の新構文を使える場面では活用
- `#[must_use]`, `#[non_exhaustive]` を意識的に付与
- `clippy` は `-W clippy::pedantic` + `-W clippy::nursery` まで上げて段階導入

**エラー / 問題発生時の作法**:
- CI 失敗、コンパイルエラー、ランタイムエラーを見た**最初の行動は公式ドキュメントを引く**こと。推測修正や stackoverflow コピペで先に手を動かさない
- 公式 docs.rs ページ、GitHub Issue、リリースノートを根拠として引用してから修正コミット
- エラーメッセージの ID (例: `error[E0308]`, `clippy::needless_borrow`) で公式 Rust error index を参照
- 修正コミットメッセージに参照元 URL を残す (「docs.rs/xxx/latest/xxx/struct.Y.html によれば…」)

### (5) 業界標準ツール積極採用 — システムで開発水準を底上げ

「必須・標準的なもの」は全て導入し、人間の規律ではなくツールで品質を担保する。

**テスト層**:
- `cargo-nextest 0.9.133+` — 並列テストランナー。業界標準
- `insta 1.38+` — snapshot testing
- `proptest 1.x` — property-based testing。パーサの不変条件 (round-trip, 冪等性) に必須
- `cargo-fuzz` — libFuzzer 統合。parser の panic-free 保証のための fuzz ハーネスを spec ごとに用意
- `criterion 0.5+` — benchmark。M1 時点でルビ/ブロック注記のパース速度 baseline を記録、regression gate

**観測性 / ログ**:
- `tracing 0.1` + `tracing-subscriber` — structured logging の de facto standard
- `tracing` span を parser のブロック境界に貼り、`RUST_LOG` で詳細制御可能に
- パース診断は `miette` の span + `tracing` の structured event の二本立て

**静的解析・セキュリティ**:
- `clippy` (pedantic + nursery) / `rustfmt` — 業界標準
- `cargo-deny 0.19.4` — ライセンス違反・vulnerability・重複依存を CI ゲート
- `cargo-audit 0.22.1` — RustSec advisory DB 連携
- `cargo-udeps` — 未使用依存検出
- `typos` — typo lint

**ドキュメント・開発フロー**:
- **ADR (Architecture Decision Records)** を `docs/adr/` に配置。MADR テンプレート使用。重要判断 (comrak fork、vendor-in-tree、Docker-only 等) を ADR として記録
- **Conventional Commits** 徹底。`commitlint` を lefthook で強制
- **release-please** でセマンティックバージョニング自動化
- `mdbook 0.4.40` でドキュメントサイト (afm-book)
- `cargo-semver-checks` で破壊的変更検出 (crates.io 公開後)

**ビルド速度**:
- `sccache` を Dockerfile に組込み、CI キャッシュ共有
- `mold` リンカを Linux で採用 (Dockerfile で構成)
- `cargo-nextest` の並列実行でテスト時間短縮

これらツールは**全て Justfile ターゲットとして固定**:
```
just test           # nextest
just prop           # proptest 駆動
just fuzz           # cargo-fuzz run (各ハーネス)
just bench          # criterion
just audit          # deny + audit + udeps
just adr <title>    # 新 ADR 作成 (テンプレ展開)
```

## その他開発環境

### pre-commit (lefthook)

`lefthook.yml` (実行自体も docker 経由):
- `just fmt-check`
- `just clippy`
- `just typos`
- `just upstream-diff` (200 行バジェット)

lefthook は Rust-native ワンバイナリなので Dockerfile の中で cargo install、Python 製 pre-commit framework は不採用。

### CI/依存管理

- `.github/workflows/ci.yml`: 全ジョブが `just <task>` を呼ぶ (CI と手元で挙動差ゼロ)
- `.github/dependabot.yml`: cargo / github-actions / npm (book) / docker / devcontainers、weekly。Actions は SHA pin + Dependabot でバンプ
- Release: `cargo-release` + `release-please`、manual tag trigger。cron/schedule ジョブは追加しない
- License: LICENSE (Apache-2.0 OR MIT デュアルで Rust 慣習) + README。REUSE spec 等の重量級フレームワークは不採用

## マイルストーン (M2 延長後)

| Phase | 期間 | 成果物 |
|---|---|---|
| **M0 Spike** | 1 週 | comrak fork プロトタイプ、ルビ `｜漢字《かんじ》` のインライン parse、hook 挿入箇所特定、`upstream-diff` xtask、`UPSTREAM_DIFF.md` 自動生成、**『罪と罰』(card 56656) ゴールデンフィクスチャで Tier A 通過** |
| **M1 Core 記法** | 2-3 週 | CommonMark + GFM (comrak そのまま通す)、ルビ、傍点 (全 7 種)、縦中横、HTML renderer、`afm-horizontal.css`、CommonMark/GFM spec CI 全合格 |
| **M2 Aozora 互換** | **4-5 週** (2週 → 延長) | `［＃...］` ブロック注記全種、gaiji 解決、`afm-encoding` crate、エイリアス正規化 (大見出し → H1)、割り注/罫囲み/割り書きの semantic HTML、120 作品 corpus Tier A+B 合格 |
| **M3 縦書き** | 1-2 週 | `afm-vertical.css`、Playwright E2E (Chromium + WebKit)、Firefox は advisory、Tier C 発動 |
| **M4 Release** | 1 週 | afm-book ドキュメント、`cargo install afm-cli`、crates.io 公開 5 crates |

**総計**: 9-12 週 (設計書の 6-9 週から延長)。ユーザーの「全記法 semantic HTML 必須」決定による。

## リスク (高→低)

1. **青空文庫 spec と実コーパスの乖離** (ほぼ確実発生): 仕様書は X だが実テキストは Y。Tier A CI で初日から検出、`Annotation{kind: Unknown}` で hard fail 回避、`docs/aozora-spec-deviations.md` で差異ログ化。
2. **comrak upstream ドリフト**: M3 時点 (2-3ヶ月後) に minor 1-2 上がる。200 行 diff バジェット + hook ポイント設計で quarterly 同期を実務 1 日程度に抑える。
3. **縦書き CSS のブラウザ互換性 (2026)**: Chromium/WebKit 安定、Firefox は縦書きルビ位置に既知バグ。`<ruby>` の semantic HTML は無条件出力、vertical CSS は opt-in、Playwright は Chromium+WebKit 必須、Firefox advisory。
4. **スコープクリープ** (pixiv記法 / なろう記法等の要望): README に out-of-scope 明記、`ExtensionOptions` で第三者拡張を受け入れ可能にし fork を誘導しない設計。
5. **C1 100% branch coverage の実現性**: error recovery ブランチが coverage-hostile で著名。対象を `aozora/**` と自前 crates のみに限定、upstream とケース的に困難な箇所は明示的除外 (理由をコミットメッセージに記録)。
6. **Shift_JIS / gaiji エッジケース**: 半角カナ、JIS X 0213 plane 2、外字マップの未掲載文字。`afm-encoding` で正規化パイプライン (SJIS → UTF-8 NFC → gaiji resolve) を組み、各段階で unit test。
7. **Corpus 取下げリスク**: 著作権関連で作品が 青空文庫 から取下げられると pin SHA が取れない。`corpus-snapshot.tar.zst` (public domain のみ vendor) をフォールバックに置く。

## 検証 (Verification)

### 機能検証 (開発時)

```bash
# ユニット+スナップショット
cargo nextest run --workspace

# CommonMark + GFM spec 互換性
cargo nextest run -p afm-parser --test commonmark_spec
cargo nextest run -p afm-parser --test gfm_spec

# 青空文庫コーパス
cargo run -p xtask -- corpus-test --tier a,b

# C1 branch coverage
cargo llvm-cov nextest --branch --workspace \
  --ignore-filename-regex 'upstream/comrak' \
  --fail-under-branches 100

# upstream diff budget
cargo run -p xtask -- upstream-diff
```

### E2E 検証 (M3 以降)

```bash
cd crates/afm-book && mdbook build
npx playwright test --project=chromium --project=webkit
```

代表入力 `examples/kokoro.md` (夏目漱石「こころ」抜粋) を縦書き HTML にレンダリングし、Chromium + WebKit で:
- ルビが base に正しく配置される
- 傍点が縦書き時は右側に出る
- 縦中横の数字が正方形配置される
- 改ページが print preview で実際にページ遷移する

### リリース前検証

```bash
cargo deny check
cargo audit
cargo publish --dry-run -p afm-syntax -p afm-encoding -p afm-parser -p afm-cli
```

## Critical Files

実装時に最初に触るファイル (優先順、**Docker 基盤を先に**):

1. `/afm/Dockerfile` (Rust 1.95 + Node 22 + cargo-nextest/llvm-cov/deny/insta + mdbook + lefthook + typos)
2. `/afm/docker-compose.yml` (dev / ci / book / browser サービス)
3. `/afm/Justfile` (全ユースケースの唯一のエントリ)
4. `/afm/bin/afm`, `/afm/bin/xtask` (docker compose run ラッパー)
5. `/afm/rust-toolchain.toml` (channel pin)
6. `/afm/Cargo.toml` (workspace)
7. `/afm/upstream/comrak/` (vendor 投入)
8. `/afm/crates/afm-syntax/src/lib.rs` (`AozoraNode` enum 定義 — 全拡張の契約 + `thiserror` エラー型)
9. `/afm/upstream/comrak/src/nodes.rs` (`NodeValue::Aozora` 1 variant 追加)
10. `/afm/crates/afm-parser/src/aozora/mod.rs` (`inline` / `block` / `html` サブモジュール + `miette` span 付き診断)
11. `/afm/upstream/comrak/src/parser/inline.rs` (ルビ hook 1 行)
12. `/afm/upstream/comrak/src/parser/block.rs` (ブロック注記 hook 1 行)
13. `/afm/upstream/comrak/src/html.rs` (renderer dispatch 1 行)
14. `/afm/upstream/comrak/src/lib.rs` (`ExtensionOptions.aozora`)
15. `/afm/crates/afm-encoding/src/lib.rs` (SJIS + gaiji + `thiserror` エラー型)
16. `/afm/crates/xtask/src/main.rs` (upstream-diff, corpus-refresh, upstream-sync)
17. `/afm/.github/workflows/ci.yml` (全ジョブが `just <task>` を呼ぶ)
18. `/afm/.devcontainer/devcontainer.json` (docker-compose dev サービスに接続)
19. `/afm/lefthook.yml` (pre-commit、実行自体も docker 経由)
