# afm workspace task runner.
# The ONE entry point for every development operation. Every target runs inside Docker;
# never invoke cargo, mdbook, or playwright on the host directly.

set shell := ["bash", "-euo", "pipefail", "-c"]
set dotenv-load := false

# --- internal helpers ---------------------------------------------------------

# Default run prefix for the interactive dev container (TTY attached)
_dev := "docker compose run --rm dev"
# Non-interactive variant for CI-like invocations (no TTY)
_ci  := "docker compose run --rm --no-TTY ci"
# Nightly-bearing variant. The `dev` image is stable-only after the Dockerfile
# fuzz-stage split; `_fuzz` is for recipes that need `cargo +nightly`
# (`udeps`, every `fuzz*` recipe, `coverage-branch`).
_fuzz := "docker compose run --rm fuzz"

# --- metadata -----------------------------------------------------------------

# Default: show this help
default:
    @just --list --unsorted

# --- build/shell --------------------------------------------------------------

# Fastest possible "does it still compile" gate. Skips codegen and
# linking; runs in seconds on a warm cache. Use as the first thing you
# run after editing source — every other build/test recipe depends on
# this being green, so failing here surfaces the problem 10× sooner than
# waiting for `just test` to error out at the same site.
check:
    {{_dev}} cargo check --workspace --all-targets

# Build all workspace crates
build:
    {{_dev}} cargo build --workspace --all-targets

# Build rustdoc for every workspace crate, exercising the
# `broken_intra_doc_links = "deny"` lint that lives in `[workspace.lints
# .rustdoc]`. Running this on every PR catches dead doc-links *before*
# docs.yml fails post-merge — the failure mode that bit us on the
# May-4 docs run and again on PR #27's merge to main, both fixed
# reactively in PR #28.
doc:
    {{_dev}} cargo doc --workspace --no-deps --document-private-items

# Build release binaries
build-release:
    {{_dev}} cargo build --release --workspace

# Drop into an interactive dev shell
shell:
    {{_dev}} bash

# Run the afm CLI with arbitrary args (same as ./bin/afm ARGS)
run *ARGS:
    {{_dev}} cargo run --package afm-cli --quiet -- {{ARGS}}

# --- tests --------------------------------------------------------------------

# Run the full test suite (unit + integration + snapshot)
test *ARGS:
    {{_dev}} cargo nextest run --workspace --all-targets {{ARGS}}

# Run doctests (nextest skips these by design)
test-doc:
    {{_dev}} cargo test --workspace --doc

# Property-based tests only. Default 128 cases per proptest block
# (AOZORA_PROPTEST_CASES override via aozora-test-utils::config). Fast
# enough to live in `just ci` — see `just prop-deep` for a stress run.
prop:
    {{_dev}} cargo nextest run --workspace --all-features --test 'property_*' --run-ignored default

# Deep property sweep — 4096 cases per block, used before cutting a
# release to exercise invariants beyond the default CI budget.
prop-deep:
    {{_dev}} bash -c 'AOZORA_PROPTEST_CASES=4096 cargo nextest run --workspace --all-features --test "property_*" --run-ignored default'

# Unit-test-only predicate pinning — runs every `invariant_unit_` test
# in `afm_parser::test_support`. Narrow target for regression hunts
# that don't need the full proptest sweep.
invariants:
    {{_dev}} cargo nextest run --package afm-markdown --lib -E 'test(invariant_unit_)'

# CommonMark 0.31.2 spec compliance (652 cases, pass = 652/652)
spec-commonmark:
    {{_dev}} cargo nextest run --package afm-markdown --test commonmark_spec

# GitHub Flavored Markdown spec compliance
spec-gfm:
    {{_dev}} cargo nextest run --package afm-markdown --test gfm_spec

# Aozora-layer fixtures (annotation cases, golden 56656, corpus sweep)
# now live in the sibling `aozora` repo; run `just spec-aozora`
# / `just spec-golden-56656` / `just corpus-sweep` from there.

# --- fuzzing -----------------------------------------------------------------
#
# The `parse_render` / `serialize_round_trip` / `sjis_decode` harnesses live in
# `crates/afm-markdown/fuzz/`. They run libFuzzer under nightly rustc inside
# the dev container.
#
# Workflow:
#   1. `just fuzz-quick TARGET` (60 s) — smoke run for development loops.
#   2. `just fuzz-deep TARGET`  (5 min) — release-gate run.
#   3. When a crash surfaces, libFuzzer writes the input to
#      `fuzz/artifacts/<target>/crash-<sha>` automatically.
#   4. `just fuzz-triage TARGET` reproduces every artifact in turn and
#      prints its Debug-formatted bytes + the panic message — analysis
#      becomes one shell call instead of N manual reproductions.
#   5. `just fuzz-promote TARGET ARTIFACT` lifts a triaged artifact into
#      `crates/afm-markdown/tests/fuzz_regressions/<target>/` so the
#      `tests/fuzz_regressions.rs` integration test treats it as a
#      permanent regression case (runs on every `just test`, no nightly
#      required). After fixing the underlying bug, `just test` is the
#      only thing the contributor needs to verify the fix in CI.

# Run the named fuzz target with arbitrary args (escape hatch for advanced use).
fuzz *ARGS:
    {{_fuzz}} bash -c 'cd crates/afm-markdown && cargo +nightly fuzz run {{ARGS}}'

# 60-second smoke fuzz — fits inside a development inner loop.
# `timeout` wraps libFuzzer as a hard backstop: if `-max_total_time`
# fires correctly the wrapper exits with libFuzzer's status; if
# libFuzzer ever hangs (rare on a busy CI runner), SIGTERM lands at
# the wrapper deadline and SIGKILL 10 s later so control returns to
# the caller in known time.
fuzz-quick TARGET:
    {{_fuzz}} bash -c 'cd crates/afm-markdown && timeout --kill-after=10s 90s cargo +nightly fuzz run {{TARGET}} -- -max_total_time=60'

# 5-minute deep fuzz — the gate to clear before tagging a release.
fuzz-deep TARGET:
    {{_fuzz}} bash -c 'cd crates/afm-markdown && timeout --kill-after=10s 360s cargo +nightly fuzz run {{TARGET}} -- -max_total_time=300'

# 15-minute marathon fuzz — the strongest single-target soak we run by
# hand. Reach for this when the 5-minute deep gate has been clean for a
# full development cycle and you want to push the corpus another order
# of magnitude. The harness exits cleanly after 15 min regardless of
# whether new paths were found.
fuzz-marathon TARGET:
    {{_fuzz}} bash -c 'cd crates/afm-markdown && timeout --kill-after=10s 1000s cargo +nightly fuzz run {{TARGET}} -- -max_total_time=900'

# Reproduce every artifact under `fuzz/artifacts/<target>/` and print
# (bytes, panic-message) for each. Exit status is the count of artifacts
# that still crash, so this can drive a CI gate. Order is alphabetical
# by hash so output stays stable across machines.
fuzz-triage TARGET:
    #!/usr/bin/env bash
    set -euo pipefail
    target="{{TARGET}}"
    art_dir="crates/afm-markdown/fuzz/artifacts/${target}"
    if [[ ! -d "$art_dir" ]]; then
        echo "fuzz-triage: no artifacts for target ${target}"
        exit 0
    fi
    failed=0
    for art in $(find "$art_dir" -type f -name 'crash-*' -o -name 'leak-*' -o -name 'oom-*' | sort); do
        # `cargo fuzz run` resolves relative paths against the crate's
        # own directory (we cd into `crates/afm-markdown` before
        # invoking it), so strip only the `crates/afm-markdown/`
        # prefix — `fuzz/artifacts/...` is the form cargo-fuzz wants.
        rel="${art#crates/afm-markdown/}"
        echo "==> ${rel}"
        out=$({{_fuzz}} bash -c "cd crates/afm-markdown && cargo +nightly fuzz run ${target} ${rel} 2>&1" || true)
        # Slice out the panic block: from the `thread … panicked` line
        # through the line just before the stack trace begins. That is
        # exactly where `assert_html_invariants` prints its tier label
        # + src + html + details — the only four lines a developer
        # actually reads. If no panic block is present, fall back to
        # the tail of the output so we never go silent.
        panic_block=$(awk '
            /^thread .* panicked at/ { capturing = 1 }
            capturing {
                if (/^stack backtrace:/ || /^=================/) exit
                print
            }
        ' <<<"$out")
        if [[ -n "$panic_block" ]]; then
            printf "%s\n" "$panic_block"
        else
            tail -5 <<<"$out"
        fi
        if grep -q "exit status: 77" <<<"$out"; then
            failed=$((failed + 1))
        fi
        echo
    done
    if (( failed > 0 )); then
        echo "fuzz-triage: ${failed} artifact(s) still crash" >&2
        exit "${failed}"
    fi
    echo "fuzz-triage: every artifact replays cleanly"

# Lift a fuzz artifact into the permanent regression set so the
# `tests/fuzz_regressions.rs` integration test asserts it forever.
# Drop the matching entry from `fuzz/artifacts/` once promoted (a
# regression case lives in tests/, not in libFuzzer's working set).
fuzz-promote TARGET ARTIFACT:
    #!/usr/bin/env bash
    set -euo pipefail
    src="crates/afm-markdown/fuzz/artifacts/{{TARGET}}/{{ARTIFACT}}"
    dst_dir="crates/afm-markdown/tests/fuzz_regressions/{{TARGET}}"
    if [[ ! -f "$src" ]]; then
        echo "fuzz-promote: artifact not found: $src" >&2
        exit 1
    fi
    # The artifact was written by libFuzzer running as root inside the
    # dev container, so the move + rm must go back through the
    # container too — host-side permissions can't unlink it.
    {{_fuzz}} bash -c "mkdir -p '$dst_dir' && mv '$src' '$dst_dir/{{ARTIFACT}}'"
    echo "promoted ${src} -> ${dst_dir}/{{ARTIFACT}}"

# Run every registered fuzz target in turn for 60 s each. Smoke pass:
# typically used after touching anything in `crates/afm-markdown/src/`
# or `crates/afm-markdown-test-support/src/`.
fuzz-all-quick:
    just fuzz-quick parse_render
    just fuzz-quick serialize_round_trip
    just fuzz-quick sjis_decode

# Run every registered fuzz target in turn for 5 min each. Release
# pre-flight pass: a clean run is the gate before tagging a release.
fuzz-all-deep:
    just fuzz-deep parse_render
    just fuzz-deep serialize_round_trip
    just fuzz-deep sjis_decode

# At-a-glance health check: how many crash artifacts are pending
# triage, how many regression cases are pinned per target. Nothing
# here invokes nightly, so it stays cheap and shell-friendly.
fuzz-status:
    #!/usr/bin/env bash
    set -euo pipefail
    targets=(parse_render serialize_round_trip sjis_decode)
    printf "%-22s  %-10s  %-12s\n" target pending_crashes pinned_regressions
    printf "%-22s  %-10s  %-12s\n" ---------------------- ---------- ------------
    for t in "${targets[@]}"; do
        crashes=0
        regressions=0
        if [[ -d "crates/afm-markdown/fuzz/artifacts/${t}" ]]; then
            crashes=$(find "crates/afm-markdown/fuzz/artifacts/${t}" -maxdepth 1 -type f \( -name 'crash-*' -o -name 'leak-*' -o -name 'oom-*' \) 2>/dev/null | wc -l | tr -d ' ')
        fi
        if [[ -d "crates/afm-markdown/tests/fuzz_regressions/${t}" ]]; then
            regressions=$(find "crates/afm-markdown/tests/fuzz_regressions/${t}" -maxdepth 1 -type f ! -name '*.txt' ! -name '*.md' 2>/dev/null | wc -l | tr -d ' ')
        fi
        printf "%-22s  %-10s  %-12s\n" "$t" "$crashes" "$regressions"
    done

# Benchmarks (criterion)
bench *ARGS:
    {{_dev}} cargo bench --workspace {{ARGS}}

# --- coverage -----------------------------------------------------------------

# Coverage gate. Fails when region coverage drops below `_COV_FLOOR`.
#
# Tool / metric rationale:
# - `cargo-llvm-cov` 0.8.5 supports `--fail-under-regions` and
#   `--fail-under-lines` / `--fail-under-functions`, but not
#   `--fail-under-branches` (the flag simply does not exist in this
#   version). Regions are a strictly finer-grained unit than branches:
#   every conditional in Rust produces separate regions for each
#   outcome, plus finer internal splits. Passing a given region
#   threshold therefore implies at least that branch threshold —
#   region coverage is an honest, stable-toolchain proxy for C1.
# - `--branch` emits branch-level counts only on nightly rustc. We stay
#   on stable for the CI gate (see `rust-toolchain.toml`) and use
#   `coverage-branch` below for informational branch reporting.
#
# Scope excludes:
# - `upstream/comrak/` — vendored fork (ADR-0001), never measured here.
# - `target/` — build artefacts.
# - `**/main.rs` — CLI binary entrypoints (`afm-cli`, `xtask`). These
#   are thin shells over their crate libraries; wiring integration
#   tests against the process entry is follow-up work.
# - `xtask/` — internal developer tooling, not a production concern.
#
# `_COV_FLOOR` is the enforced minimum today, not the goal. The
# stated goal (ADR-0006 §coverage) is 100% on production code. The
# floor ratchets upward in follow-up commits that close specific
# gaps; see task tracker.
#
# Ratchet history:
# - 94 (M0): initial gate landing with the parse pipeline wired up.
# - 95 (G3): F-series fills in recogniser branches + G1 adds full
#   gaiji resolve coverage; measured regions total 95.36%.
# - 96 (Cov-Ratchet): diagnostic.rs V1/V2/V3 constructor tests +
#   phase6_validate.rs synthetic-registry tests + html.rs
#   section/container/double-ruby unit tests; measured 96.07%.
#   Remaining gaps are mostly non-exhaustive `_` arms on
#   `#[non_exhaustive]` enums (structurally unreachable in-crate).
# 93 (Aozora-Split, post-v0.2.0): afm-parser → afm-markdown rename
#   moved the lexer/AST/encoding tests out into the sibling aozora
#   repo. The afm-markdown test surface is now ~half its previous
#   size; total regions dropped from 96.07% to 93.44%. The 96% gate
#   was set against a workspace that included those crates, so we
#   ratchet down to the new measurement and tighten back up as the
#   afm-markdown-specific test suite grows (post_process invariants,
#   aozora_parity differential, html-shape proptests).
# 96 (post-v0.2.5 production-only): test_support.rs was extracted
#   from the coverage measurement because it is `#[doc(hidden)]
#   pub mod` shipped only so integration tests in `tests/*.rs` can
#   share invariant helpers — it is not production code. The
#   measured surface is now lib.rs / post_process.rs / html.rs.
# 96 (IR + streaming + WASM): afm-wasm exposes wasm-bindgen entry
#   points that native `cargo llvm-cov` cannot exercise, so the
#   crate is permanently excluded from measurement (its surface is
#   exercised by `wasm-pack test` instead, separately from the
#   coverage gate). The IR walker (ir.rs) and the streaming /
#   anchor paths (lib.rs) are exercised by `tests/ir_coverage.rs`
#   to keep production coverage above the 96 floor.
_COV_FLOOR := "96"
_COV_IGNORE := "(upstream/comrak|target/|/main\\.rs$|xtask/|afm-markdown-test-support/|afm-wasm/)"

coverage:
    {{_dev}} cargo llvm-cov nextest \
        --workspace \
        --ignore-filename-regex '{{_COV_IGNORE}}' \
        --fail-under-regions {{_COV_FLOOR}}

# HTML coverage report for local inspection. No threshold — intended
# for opening `coverage/html/index.html` in a browser.
coverage-html:
    {{_dev}} cargo llvm-cov nextest \
        --workspace \
        --ignore-filename-regex '{{_COV_IGNORE}}' \
        --html --output-dir coverage/html

# Branch-level coverage report (requires nightly for `--branch` support).
# Informational only — no threshold. Use to surface uncovered conditionals
# when working a specific file toward C1 100%.
coverage-branch:
    {{_fuzz}} cargo +nightly llvm-cov nextest \
        --branch \
        --workspace \
        --ignore-filename-regex '{{_COV_IGNORE}}'

# --- lint / static analysis ---------------------------------------------------

# Run all lints (fmt + clippy + typos + strict-code)
lint: fmt-check clippy typos strict-code

# Forbid patterns that hide bugs or introduce unstable/unsafe surface in our
# own crates. upstream/comrak is excluded (ADR-0001 keeps vendored tree
# untouched). Every check is defensive — each represents a pattern we have
# decided IS a bug-source and want rejected at the gate rather than fought
# later in code review.
strict-code:
    #!/usr/bin/env bash
    set -euo pipefail
    shopt -s globstar
    files=(crates/**/*.rs)

    check() {
        local label="$1"
        local pattern="$2"
        local hits
        hits=$(grep -nE "$pattern" "${files[@]}" 2>/dev/null || true)
        if [[ -n "$hits" ]]; then
            echo "==> forbidden: $label" >&2
            echo "$hits" >&2
            return 1
        fi
    }

    failed=0

    # ---- Warning suppression -----------------------------------------------
    # #[allow(...)] / #![allow(...)] / #[cfg_attr(..., allow(...))]
    # accumulate dead rules that hide real bugs. Memory note
    # feedback_no_warning_suppression: refactor the code instead.
    check 'warning suppression (#[allow] / cfg_attr+allow)' \
        '^\s*(#!?\[allow\(|#!?\[cfg_attr\([^)]*allow\()' || failed=1

    # ---- Nightly / unstable feature gates ----------------------------------
    # We ship on Rust stable only. Feature gates silently tie us to nightly
    # and rot on toolchain bumps.
    check 'nightly feature gate (#[feature] / #![feature])' \
        '^\s*#!?\[feature\(' || failed=1

    # ---- Unsafe code -------------------------------------------------------
    # Every crate root has `#![forbid(unsafe_code)]` (checked below); this
    # text-level grep is belt-and-braces for typos that would defeat the
    # compiler gate. Excludes the legitimate `r#unsafe` raw-identifier form
    # used by comrak's `render.r#unsafe` field.
    check 'unsafe code (unsafe fn / unsafe { / unsafe impl / unsafe trait)' \
        '(^|[^a-zA-Z_#])unsafe\s+(fn|impl|trait|\{)' || failed=1

    # ---- Required deny directive -------------------------------------------
    # Each crate root must start with `#![forbid(unsafe_code)]` so accidental
    # unsafe additions are rejected at compile time.
    for root in crates/*/src/lib.rs crates/*/src/main.rs; do
        [[ -f "$root" ]] || continue
        if ! grep -q '^#!\[forbid(unsafe_code)\]' "$root"; then
            echo "==> forbidden: crate root missing '#![forbid(unsafe_code)]'" >&2
            echo "  $root" >&2
            failed=1
        fi
    done

    # ---- Toolchain pinning -------------------------------------------------
    # rust-toolchain.toml must pin a semver-numbered stable channel. Any
    # appearance of nightly/beta in the channel pin is rejected.
    if grep -qE '^\s*channel\s*=\s*"(nightly|beta)' rust-toolchain.toml; then
        echo "==> forbidden: rust-toolchain.toml pins a pre-stable channel" >&2
        grep -nE '^\s*channel' rust-toolchain.toml >&2
        failed=1
    fi

    # ---- TODO/FIXME/XXX without an issue reference -------------------------
    # Drive-by notes rot into dead reminders. Every TODO/FIXME/XXX must
    # reference either an issue (`#N`) or a milestone (`M1..M4`) so it can
    # be tracked or reclassified. Requires word-boundary match so placeholder
    # hex sequences like `U+XXXX` don't false-positive.
    todo_hits=$(grep -nE '(^|[^[:alnum:]_])(TODO|FIXME|XXX)([^[:alnum:]_]|$)' "${files[@]}" 2>/dev/null \
        | grep -vE '(#[0-9]+|M[0-9]|issue|ADR-[0-9]+)' || true)
    if [[ -n "$todo_hits" ]]; then
        echo '==> forbidden: bare TODO/FIXME/XXX without an issue or milestone reference' >&2
        echo "$todo_hits" >&2
        failed=1
    fi

    # ---- println! / eprintln! in library crates ----------------------------
    # Library crates should emit observability via `tracing`, not raw print.
    # CLI crates (afm-cli, xtask) are expected to print, so they are scoped
    # out. Examples (`crates/*/examples/`) and fuzz targets
    # (`crates/*/fuzz/fuzz_targets/`) are also exempt — they're binary-style
    # demos, not library code. This complements clippy::print_stdout /
    # clippy::print_stderr, which cannot be selectively enabled per-crate
    # while still inheriting [workspace.lints] (rust-lang/cargo#12697).
    lib_files=(crates/afm-markdown/**/*.rs)
    print_hits=$(grep -nE '(^|[^[:alnum:]_])e?print(ln)?!\s*\(' "${lib_files[@]}" 2>/dev/null \
        | grep -vE '/(tests|benches|examples|fuzz_targets)/' || true)
    if [[ -n "$print_hits" ]]; then
        echo '==> forbidden: println! / eprintln! in library crates (use tracing instead)' >&2
        echo "$print_hits" >&2
        failed=1
    fi

    if [[ $failed -ne 0 ]]; then
        echo "" >&2
        echo "strict-code check failed. Refactor the offending sites; do not silence." >&2
        exit 1
    fi
    echo "strict-code: clean"

# Format check (no-write)
fmt-check:
    {{_dev}} cargo fmt --all -- --check

# Auto-format (writes)
fmt:
    {{_dev}} cargo fmt --all

# Clippy — lint groups (pedantic/nursery/cargo) and carve-outs are owned
# entirely by `[workspace.lints]` in Cargo.toml. Passing `-W clippy::<group>`
# here would re-enable the whole group at CLI priority and silently undo
# per-lint allow carve-outs (e.g. `redundant_pub_crate`). Keep the CLI
# surface to `-D warnings` only.
clippy:
    {{_dev}} cargo clippy --workspace --all-targets --all-features -- -D warnings

# Typo check
typos:
    {{_dev}} typos

# Assert that tool-version pins which live in multiple files agree.
# bun is pinned in three locations (Dockerfile / playground/package.json
# / docs.yml); wasm-pack in two (Dockerfile / docs.yml). A patch bump
# that only touches one would let dev and CI silently resolve different
# versions. This recipe greps each file for the canonical pattern and
# fails (exit 1) if any pair disagrees — the mechanical replacement for
# a "remember to update all three" comment.
verify-version-pins:
    #!/usr/bin/env bash
    set -euo pipefail
    fail=0
    extract() {
        local file="$1"
        local pattern="$2"
        grep -oE "$pattern" "$file" | head -1 | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' || true
    }

    # bun: Dockerfile ARG / playground/package.json packageManager / docs.yml setup-bun
    bun_docker=$(extract Dockerfile 'BUN_VERSION=[0-9.]+')
    bun_pkg=$(extract playground/package.json '"bun@[0-9.]+"')
    bun_docs=$(extract .github/workflows/docs.yml "bun-version: '[0-9.]+'")
    if [[ -n "$bun_docker" && "$bun_docker" == "$bun_pkg" && "$bun_docker" == "$bun_docs" ]]; then
        printf '[OK] bun pin: %s (Dockerfile / playground/package.json / docs.yml agree)\n' "$bun_docker"
    else
        printf '[!!] bun pin drift: Dockerfile=%s playground/package.json=%s docs.yml=%s\n' \
            "$bun_docker" "$bun_pkg" "$bun_docs" >&2
        fail=1
    fi

    # wasm-pack: Dockerfile ARG / docs.yml jetli/wasm-pack-action
    wp_docker=$(extract Dockerfile 'WASM_PACK_VERSION=[0-9.]+')
    wp_docs=$(extract .github/workflows/docs.yml "version: 'v[0-9.]+'")
    if [[ -n "$wp_docker" && "$wp_docker" == "$wp_docs" ]]; then
        printf '[OK] wasm-pack pin: %s (Dockerfile / docs.yml agree)\n' "$wp_docker"
    else
        printf '[!!] wasm-pack pin drift: Dockerfile=%s docs.yml=%s\n' \
            "$wp_docker" "$wp_docs" >&2
        fail=1
    fi

    if (( fail == 0 )); then
        echo "verify-version-pins: all pins agree"
        exit 0
    else
        echo "verify-version-pins: drift detected — see [!!] lines above" >&2
        exit "$fail"
    fi

# Dependency linting (licenses, advisories, bans)
deny:
    {{_dev}} cargo deny check

# RustSec advisory scan
audit:
    {{_dev}} cargo audit

# Unused dependency scan (requires nightly)
udeps:
    {{_fuzz}} cargo +nightly udeps --workspace --all-targets

# Semver break detection (runs against published baseline once crates are on crates.io)
semver:
    {{_dev}} cargo semver-checks check-release --workspace

# --- upstream / fork management ----------------------------------------------

# Report diff-line count against upstream comrak (hard fail > 200 lines)
upstream-diff:
    {{_dev}} cargo run --package xtask --quiet -- upstream-diff

# Sync upstream comrak to TAG and re-apply hook patches
upstream-sync TAG:
    {{_dev}} cargo run --package xtask --quiet -- upstream-sync {{TAG}}

# Pin every `aozora-*` git dep in Cargo.toml to a new commit SHA in one
# pass, then refresh Cargo.lock. Idempotent (no-op when the SHA already
# matches). Use the full 40-char hex SHA from `git ls-remote
# https://github.com/P4suta/aozora.git refs/heads/main`.
aozora-bump SHA:
    {{_dev}} cargo run --package xtask --quiet -- aozora-bump {{SHA}}

# Regenerate `spec/*.json` from the vendored cmark-format sources under
# `spec/sources/*.txt`. Offline-pure: both the sources and the generated
# fixtures are committed to the repo. Add new `spec/sources/<name>.txt`
# files and extend the conversion block below to cover them.
spec-refresh:
    {{_dev}} bash -c '\
        set -euo pipefail && \
        cargo run --package xtask --quiet -- spec-refresh \
            --input spec/sources/commonmark-0.31.2.txt \
            --output spec/commonmark-0.31.2.json && \
        cargo run --package xtask --quiet -- spec-refresh \
            --input spec/sources/gfm-0.29-gfm.txt \
            --output spec/gfm-0.29-gfm.json'

# --- docs ---------------------------------------------------------------------

# Build the mdbook documentation site
book-build:
    docker compose run --rm book mdbook build

# Serve the mdbook site at http://localhost:3000
book-serve:
    docker compose up book

# Check documentation links
book-linkcheck:
    docker compose run --rm book mdbook-linkcheck

# New Architecture Decision Record (MADR template)
adr TITLE:
    {{_dev}} cargo run --package xtask --quiet -- new-adr {{TITLE}}

# Regenerate crates/afm-wasm/types/afm_types.d.ts from the live IR +
# wasm envelope types. Commit the diff so `types-check` stays green.
types:
    {{_dev}} cargo run --package xtask --quiet -- types ts

# Drift gate: fail if the committed afm_types.d.ts disagrees with fresh
# codegen. Wired into `just ci` (and the `types-check` CI job); run after
# touching the IR types.
types-check:
    {{_dev}} cargo run --package xtask --quiet -- types check

# Regenerate CHANGELOG.md from Conventional-Commits history (see cliff.toml).
# Uses git-cliff inside the dev container — the tool is provisioned by the
# Dockerfile's cargo-tools stage, so `just changelog` should work on any
# developer machine after the initial image build.
changelog:
    {{_dev}} git-cliff -o CHANGELOG.md

# --- end-to-end (M3 onward) --------------------------------------------------

# Playwright browser tests (Chromium + WebKit)
e2e *ARGS:
    docker compose run --rm browser \
        bash -c 'cd crates/afm-book && npm ci && npx playwright test {{ARGS}}'

# --- playground (browser try-it-online) --------------------------------------

# Vite dev/preview server container — `--service-ports` is required so
# `docker compose run` actually publishes 5173 (it doesn't by default).
_pg := "docker compose run --rm --service-ports playground"

# Same container without publishing 5173. Used by `playground-install`
# and `playground-build` so they share the `playground-node-modules`
# named volume but don't trip "address already in use" when an existing
# Vite or dev server is bound to 5173 on the host.
_pg_install := "docker compose run --rm playground"

# Build the afm-wasm package consumed by the playground (and any browser host).
# Output lands in `crates/afm-wasm/pkg/`, referenced by `playground/package.json`
# via `"afm-wasm": "file:../crates/afm-wasm/pkg"`.
#
# `RUSTC_WRAPPER=` (empty) bypasses sccache for the wasm-pack invocation:
# wasm-pack internally triggers `rustup target add wasm32-unknown-unknown`
# and the rustup-sync subprocess scrubs SCCACHE_GHA_ENABLED to an invalid
# value, which sccache 0.15+ rejects with "must be 'true', 'on', ...". The
# wasm build is per-target (separate target dir from native) so the cache
# benefit was marginal anyway.
wasm-build:
    {{_dev}} bash -c 'RUSTC_WRAPPER= wasm-pack build crates/afm-wasm \
        --target bundler --release \
        --out-dir pkg --out-name afm_wasm'

# Dev-profile wasm build for playground iteration. Skips wasm-opt and uses
# the `dev` cargo profile; output is 3-5× bigger and slower at runtime but
# completes in ~10-20 s vs the 60-90 s `wasm-build` release path. Do NOT
# ship the output to GitHub Pages — `just playground-build` and the docs
# workflow both use the release `wasm-build` recipe instead.
wasm-build-dev:
    {{_dev}} bash -c 'RUSTC_WRAPPER= wasm-pack build crates/afm-wasm \
        --target bundler --dev \
        --out-dir pkg --out-name afm_wasm'

# Install playground deps via bun. Depends on `wasm-build` because the
# `file:` link requires the target directory to exist before `bun install`
# resolves it. Runs inside the `playground` service (no published ports)
# so `node_modules` lands in the named volume (`playground-node-modules`)
# instead of the host bind mount — important on Docker Desktop / WSL
# where cross-fs writes are slow.
playground-install: wasm-build
    {{_pg_install}} bash -c 'bun install'

# Vite dev server with HMR at http://localhost:5173/
playground-dev: playground-install
    {{_pg}} bash -c 'bun run dev -- --host 0.0.0.0'

# Same as `playground-dev` but uses the fast dev-profile wasm build for
# inner-loop iteration (TS edits get HMR; wasm changes still need a
# reload after `just wasm-build-dev`).
playground-dev-fast: wasm-build-dev
    {{_pg_install}} bash -c 'bun install' && \
    {{_pg}} bash -c 'bun run dev -- --host 0.0.0.0'

# Production build → playground/dist/ (consumed by .github/workflows/docs.yml)
# Also runs inside `playground` service to share the `node_modules` volume.
playground-build: playground-install
    {{_pg_install}} bash -c 'bun run build'

# Preview the production build locally at http://localhost:5173/
playground-serve: playground-build
    {{_pg}} bash -c 'bun run preview -- --host 0.0.0.0 --port 5173'

# --- aggregate ----------------------------------------------------------------

# Local replica of the full CI pipeline. Ordered fail-fast: cheap checks
# first, slow ones last. Every step prints its own `[HH:MM:SS] →→→ name`
# start banner and `✓ name (took Ns)` or `✗ name FAILED (after Ns)`
# trailer, so a long run never leaves the user wondering whether it's
# stuck or progressing. A failure exits immediately with the failing
# step's exit code; no downstream work runs.
ci:
    #!/usr/bin/env bash
    set -uo pipefail

    # Step ordering rationale:
    #   1-3  no-compile static checks (seconds; fastest signal)
    #   4    grep-based source rules (fast)
    #   5    verify-version-pins — catches Dockerfile / docs.yml / package.json drift
    #   6    cargo check (typecheck only; warm-cache fast)
    #   6b   types-check — IR→TS `.d.ts` codegen drift gate (needs xtask compiled)
    #   7    cargo doc — exercises `broken_intra_doc_links = deny`; the
    #        only place this lint actually runs (check / clippy skip it)
    #   8-9  Cargo.lock-only checks (no compile required)
    #   10   clippy via `lint` composite (heavy lint pass — full compile)
    #   11   build (validate all targets compile)
    #   12-15 test pyramid — unit → property → spec
    #   16   coverage (instrumented compile, slow)
    #   17   book — independent of cargo state
    #   18   udeps — nightly only; deferred so a stable failure surfaces first
    steps=(
        "typos"
        "fmt-check"
        "upstream-diff"
        "strict-code"
        "verify-version-pins"
        "check"
        "types-check"
        "doc"
        "deny"
        "audit"
        "lint"
        "build"
        "test"
        "prop"
        "spec-commonmark"
        "spec-gfm"
        "coverage"
        "book-build"
        "udeps"
    )

    total=${#steps[@]}
    i=0
    pipeline_start=$(date +%s)
    for step in "${steps[@]}"; do
        i=$((i + 1))
        printf '\n\033[1;36m[%s] →→→ STEP %d/%d: %s\033[0m\n' \
            "$(date +%T)" "$i" "$total" "$step"
        start=$(date +%s)
        if just "$step"; then
            end=$(date +%s)
            printf '\033[1;32m[%s] ✓ %s (took %ds)\033[0m\n' \
                "$(date +%T)" "$step" $((end - start))
        else
            rc=$?
            end=$(date +%s)
            printf '\n\033[1;31m[%s] ✗ %s FAILED (after %ds, exit %d)\033[0m\n' \
                "$(date +%T)" "$step" $((end - start)) "$rc"
            printf '\033[1;31mPipeline halted at step %d/%d. %d step(s) remained.\033[0m\n' \
                "$i" "$total" $((total - i))
            exit "$rc"
        fi
    done
    pipeline_end=$(date +%s)
    printf '\n\033[1;32m[%s] ✓✓✓ all %d steps passed (total %ds)\033[0m\n' \
        "$(date +%T)" "$total" $((pipeline_end - pipeline_start))

# --- developer workflow helpers ----------------------------------------------

# Snapshot of the local environment in one screen. Tells you
# immediately which images are present, which volumes are mounted,
# whether sccache is configured, whether the aozora SHA pin in
# Cargo.toml matches Cargo.lock, and whether the playground artefacts
# are ready to serve. Exit 0 = nothing wrong; exit 1 = missing
# prerequisite a build will trip on. Run before / after major
# operations so you never wonder "is my environment broken".
doctor:
    #!/usr/bin/env bash
    set -uo pipefail
    OK="\033[1;32m[OK]\033[0m"
    WARN="\033[1;33m[--]\033[0m"
    ERR="\033[1;31m[!!]\033[0m"

    fail=0

    # --- Docker availability ---------------------------------------------
    if command -v docker >/dev/null 2>&1; then
        printf '%b docker: %s\n' "$OK" "$(docker --version | awk '{print $3}' | tr -d ,)"
    else
        printf '%b docker: NOT INSTALLED\n' "$ERR"
        fail=1
    fi
    if docker compose version >/dev/null 2>&1; then
        printf '%b docker compose: %s\n' "$OK" "$(docker compose version --short)"
    else
        printf '%b docker compose: missing (install Compose v2)\n' "$ERR"
        fail=1
    fi

    # --- Images ----------------------------------------------------------
    # docker images Go-template strings collide with just's `}}`
    # interpolator; parse the human-readable table with awk instead.
    # Output columns: REPOSITORY TAG IMAGE-ID CREATED SIZE. NR==2 picks
    # the first data row; awk's last field is the size.
    for tag in afm-dev:local afm-fuzz:local afm-ci:local; do
        size=$(docker images "$tag" 2>/dev/null | awk 'NR==2 {print $NF}')
        if [ -n "$size" ]; then
            printf '%b image %s (%s)\n' "$OK" "$tag" "$size"
        else
            case "$tag" in
                afm-dev:local)   hint='just check        # auto-builds dev' ;;
                afm-fuzz:local)  hint='docker compose build fuzz' ;;
                afm-ci:local)    hint='docker compose build ci  # superset' ;;
            esac
            printf '%b image %s missing  →  %s\n' "$WARN" "$tag" "$hint"
        fi
    done

    # --- Volumes ---------------------------------------------------------
    for vol in afm_cargo-registry afm_cargo-git afm_cargo-target afm_sccache; do
        if docker volume inspect "$vol" >/dev/null 2>&1; then
            printf '%b volume %s\n' "$OK" "$vol"
        else
            printf '%b volume %s missing (created on first compose run)\n' "$WARN" "$vol"
        fi
    done

    # --- aozora SHA pin ↔ Cargo.lock --------------------------------------
    pinned=$(grep -oE 'rev = "[0-9a-f]{40}"' Cargo.toml | head -1 | grep -oE '[0-9a-f]{40}' || true)
    if [ -n "$pinned" ]; then
        if grep -q "rev = \"$pinned\"" Cargo.lock 2>/dev/null \
            || grep -q "#${pinned:0:7}" Cargo.lock 2>/dev/null; then
            printf '%b aozora rev pin: %s (Cargo.lock agrees)\n' "$OK" "${pinned:0:12}…"
        else
            printf '%b aozora rev pin %s NOT reflected in Cargo.lock  →  cargo update -p aozora-syntax\n' \
                "$ERR" "${pinned:0:12}…"
            fail=1
        fi
    else
        printf '%b aozora rev pin: not found in Cargo.toml\n' "$ERR"
        fail=1
    fi

    # --- Playground prerequisites ----------------------------------------
    if [ -f crates/afm-wasm/pkg/afm_wasm_bg.wasm ]; then
        pkg_size=$(du -h crates/afm-wasm/pkg/afm_wasm_bg.wasm | awk '{print $1}')
        printf '%b crates/afm-wasm/pkg (%s)\n' "$OK" "$pkg_size"
    else
        printf '%b crates/afm-wasm/pkg missing  →  just wasm-build  (or just wasm-build-dev for fast iter)\n' "$WARN"
    fi
    if [ -d playground/node_modules ]; then
        printf '%b playground/node_modules\n' "$OK"
    else
        printf '%b playground/node_modules missing  →  just playground-install\n' "$WARN"
    fi

    # --- Summary ---------------------------------------------------------
    echo
    if [ "$fail" -eq 0 ]; then
        printf '\033[1;32mall blocking prerequisites satisfied\033[0m\n'
        exit 0
    else
        printf '\033[1;31m%d blocking issue(s) found — fix before continuing\033[0m\n' "$fail"
        exit 1
    fi

# Run after a build to verify the cache is actually warm; a first-hand
# way to notice when `RUSTC_WRAPPER` gets defeated by stray env or profile tweaks.
# Show sccache hit/miss ratio, cache size, fetch counts.
sccache-stats:
    {{_dev}} sccache --show-stats

# Useful before a measurement window:
#   just sccache-zero && just clean && just build && just sccache-stats
# Reset sccache counters to zero.
sccache-zero:
    {{_dev}} sccache --zero-stats

# Defaults to the `check` job; pass a job name to pick another, e.g.
# `just watch clippy`. Keybindings: `t` test / `c` clippy / `d` doc /
# `f` failing-only / `esc` previous job / `q` quit / Ctrl-J list jobs.
# Start the bacon file-watcher inside the dev container.
watch JOB="":
    {{_dev}} bacon {{JOB}}

# Keeps the watch loop but prints plain lines. Useful for piping output
# (`| tee`) and for sessions without a TTY.
# Headless bacon run (no TUI).
watch-headless JOB="check":
    {{_ci}} bacon --headless --job {{JOB}}

# Idempotent — re-run safely after lefthook.yml edits or to repair stubs.
# Install git hooks (pre-commit / commit-msg / pre-push).
hooks:
    {{_dev}} lefthook install

# Remove lefthook git hook stubs.
hooks-uninstall:
    {{_dev}} lefthook uninstall

# --- cleanup ------------------------------------------------------------------

# Remove build artifacts (keeps volumes; use `docker compose down -v` for volumes)
clean:
    {{_dev}} cargo clean --workspace

# Tear down all compose state (destroys cached registry/target/sccache volumes)
nuke:
    docker compose down -v --remove-orphans
