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

# --- metadata -----------------------------------------------------------------

# Default: show this help
default:
    @just --list --unsorted

# --- build/shell --------------------------------------------------------------

# Build all workspace crates
build:
    {{_dev}} cargo build --workspace --all-targets

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
    {{_dev}} bash -c 'cd crates/afm-markdown && cargo +nightly fuzz run {{ARGS}}'

# 60-second smoke fuzz — fits inside a development inner loop.
fuzz-quick TARGET:
    {{_dev}} bash -c 'cd crates/afm-markdown && cargo +nightly fuzz run {{TARGET}} -- -max_total_time=60'

# 5-minute deep fuzz — the gate to clear before tagging a release.
fuzz-deep TARGET:
    {{_dev}} bash -c 'cd crates/afm-markdown && cargo +nightly fuzz run {{TARGET}} -- -max_total_time=300'

# 15-minute marathon fuzz — the strongest single-target soak we run by
# hand. Reach for this when the 5-minute deep gate has been clean for a
# full development cycle and you want to push the corpus another order
# of magnitude. The harness exits cleanly after 15 min regardless of
# whether new paths were found.
fuzz-marathon TARGET:
    {{_dev}} bash -c 'cd crates/afm-markdown && cargo +nightly fuzz run {{TARGET}} -- -max_total_time=900'

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
        out=$({{_dev}} bash -c "cd crates/afm-markdown && cargo +nightly fuzz run ${target} ${rel} 2>&1" || true)
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
    {{_dev}} bash -c "mkdir -p '$dst_dir' && mv '$src' '$dst_dir/{{ARTIFACT}}'"
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
    {{_dev}} cargo +nightly llvm-cov nextest \
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

# Dependency linting (licenses, advisories, bans)
deny:
    {{_dev}} cargo deny check

# RustSec advisory scan
audit:
    {{_dev}} cargo audit

# Unused dependency scan (requires nightly)
udeps:
    {{_dev}} cargo +nightly udeps --workspace --all-targets

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

# --- aggregate ----------------------------------------------------------------

# Local replica of the full CI pipeline — everything must pass before push
ci:
    just lint
    just build
    just test
    just prop
    just spec-commonmark
    just spec-gfm
    just deny
    just audit
    just udeps
    just upstream-diff
    just coverage
    just book-build

# --- developer workflow helpers ----------------------------------------------

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
