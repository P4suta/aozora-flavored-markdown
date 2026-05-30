# syntax=docker/dockerfile:1.7
# afm development / CI container
# Every developer and CI job runs inside this image. Host toolchain is never invoked.
#
# Layered so upstream-sync / dependency bumps rebuild minimal surface.

ARG RUST_VERSION=1.95.0
ARG NODE_VERSION=22

########################################################################
# Stage: toolchain — Rust stable + system deps for builds and CJK work
########################################################################
FROM rust:${RUST_VERSION}-bookworm AS toolchain

RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt,sharing=locked \
    apt-get update && \
    apt-get install -y --no-install-recommends \
        build-essential \
        pkg-config \
        libssl-dev \
        clang \
        mold \
        curl \
        git \
        ca-certificates \
        unzip \
        xz-utils \
        locales \
    && sed -i -e 's/# \(ja_JP.UTF-8 UTF-8\)/\1/' /etc/locale.gen \
    && sed -i -e 's/# \(en_US.UTF-8 UTF-8\)/\1/' /etc/locale.gen \
    && locale-gen

ENV LANG=en_US.UTF-8 \
    LC_ALL=en_US.UTF-8 \
    RUSTUP_PERMIT_COPY_RENAME=1

# Use mold as the default linker for faster builds
RUN mkdir -p /root/.cargo && printf '%s\n' \
    '[target.x86_64-unknown-linux-gnu]' \
    'linker = "clang"' \
    'rustflags = ["-C", "link-arg=-fuse-ld=mold"]' \
    > /root/.cargo/config.toml

########################################################################
# Stage: cargo-tools — install Rust dev utilities (cached layer)
########################################################################
FROM toolchain AS cargo-tools

# cargo-binstall fetches prebuilt GitHub Release binaries for every
# subsequent tool, avoiding the 30-min source compile that the previous
# monolithic `cargo install --locked` layer cost. Only binstall itself
# is built from source (~30 s, one bin).
ARG BINSTALL_VERSION=1.15.6
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/tmp/cargo-build \
    CARGO_TARGET_DIR=/tmp/cargo-build \
    cargo install --locked --version "${BINSTALL_VERSION}" --root /usr/local cargo-binstall

# Tier A — yearly-churn tools. Cached for the longest; bumping any one
# of these is rare enough to be worth its own dedicated invalidation.
RUN cargo binstall --no-confirm --locked --root /usr/local \
        mdbook \
        mdbook-linkcheck \
        typos-cli

# Tier B — quarterly-churn tools. Test infrastructure that moves at a
# moderate cadence.
RUN cargo binstall --no-confirm --locked --root /usr/local \
        cargo-nextest \
        cargo-deny \
        cargo-audit \
        cargo-insta

# Tier C — monthly-churn tools. Heavier upstream churn — these used to
# dominate the old monolithic install's cold-build time. sccache is
# pinned to 0.10.0: 0.15+ introduces a GHA-backend probe that errors
# inside cargo's rustc-wrapper invocation path with "SCCACHE_GHA_ENABLED
# must be 'true', 'on', '1', 'false', 'off' or '0'" even when the env
# is unset and we're nowhere near GHA. Hold the pin until upstream fixes.
RUN cargo binstall --no-confirm --locked --root /usr/local \
        cargo-llvm-cov \
        cargo-semver-checks \
        sccache@0.10.0

# Tier D — as-needed release helpers. Split so a bump here doesn't
# touch the test-cycle tier.
RUN cargo binstall --no-confirm --locked --root /usr/local \
        cargo-edit \
        cargo-release

# bacon — kept in its own layer (separate churn axis from the test/lint tiers).
RUN cargo binstall --no-confirm --locked --root /usr/local bacon

# git-cliff for CHANGELOG generation — kept separate for the same reason.
RUN cargo binstall --no-confirm --locked --root /usr/local git-cliff

# just (task runner) installed separately; upstream provides an install script
RUN curl -fsSL https://just.systems/install.sh \
    | bash -s -- --to /usr/local/bin --tag 1.36.0

# lefthook (pre-commit manager). As of 2.x the release asset is a gzipped raw binary.
ARG LEFTHOOK_VERSION=2.1.6
RUN curl -fsSL \
    "https://github.com/evilmartians/lefthook/releases/download/v${LEFTHOOK_VERSION}/lefthook_${LEFTHOOK_VERSION}_Linux_x86_64.gz" \
    | gunzip > /usr/local/bin/lefthook \
    && chmod +x /usr/local/bin/lefthook

# wasm-pack — builds the afm-wasm crate consumed by `playground/` and any
# browser host. Pinned alongside the workflow pin in .github/workflows/docs.yml
# so dev and CI agree on the wasm-bindgen-cli that gets auto-fetched.
ARG WASM_PACK_VERSION=0.13.1
RUN curl -fsSL \
    "https://github.com/rustwasm/wasm-pack/releases/download/v${WASM_PACK_VERSION}/wasm-pack-v${WASM_PACK_VERSION}-x86_64-unknown-linux-musl.tar.gz" \
    | tar -xz -C /usr/local/bin --strip-components=1 \
        "wasm-pack-v${WASM_PACK_VERSION}-x86_64-unknown-linux-musl/wasm-pack"

# bun — JavaScript runtime + package manager for the playground (TS edits,
# Vite dev server, production build). Node 22 stays in the dev image for
# the book/playwright services that still consume npm tooling. The pin
# below must agree with `playground/package.json` "packageManager" and
# `.github/workflows/docs.yml` setup-bun `bun-version:` — `just verify-
# version-pins` is the mechanical gate that catches drift.
ARG BUN_VERSION=1.3.14
RUN curl -fsSL \
    "https://github.com/oven-sh/bun/releases/download/bun-v${BUN_VERSION}/bun-linux-x64.zip" \
    -o /tmp/bun.zip \
    && unzip -d /tmp /tmp/bun.zip \
    && mv "/tmp/bun-linux-x64/bun" /usr/local/bin/bun \
    && chmod +x /usr/local/bin/bun \
    && rm -rf /tmp/bun.zip /tmp/bun-linux-x64

########################################################################
# Stage: node — Node.js 22 for mdbook plugins & Playwright (used by book/browser)
########################################################################
FROM toolchain AS node-base

ARG NODE_VERSION
RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt,sharing=locked \
    curl -fsSL https://deb.nodesource.com/setup_${NODE_VERSION}.x | bash - && \
    apt-get install -y --no-install-recommends nodejs && \
    corepack enable

########################################################################
# Stage: dev — stable-only contributor image (no nightly, no fuzz/udeps)
########################################################################
FROM node-base AS dev

COPY --from=cargo-tools /usr/local/cargo/bin/ /usr/local/cargo/bin/
COPY --from=cargo-tools /usr/local/bin/ /usr/local/bin/

# Pre-install the components `rust-toolchain.toml` requires (rustfmt,
# clippy, rust-src). Without this, the first `cargo` invocation from
# inside /workspace triggers an automatic `rustup` channel sync that
# spawns subprocesses with a scrubbed env — `SCCACHE_GHA_ENABLED`
# arrives empty and sccache 0.15+ aborts with "must be 'true', 'on',
# '1', 'false', 'off' or '0'". Installing them at image build time
# means the sync never fires.
RUN rustup component add rustfmt clippy rust-src

ENV CARGO_HOME=/cargo/home \
    CARGO_TARGET_DIR=/cargo/target \
    RUSTC_WRAPPER=sccache \
    SCCACHE_DIR=/cargo/sccache \
    RUST_BACKTRACE=1

# Pre-create the cache mount targets at /cargo/* so the named volume
# mounts attach cleanly. These live OUTSIDE the /workspace bind mount
# on purpose (see docker-compose.yml): nesting them under /workspace
# made the daemon create root-owned ./target / ./.cargo / ./.sccache
# on the host, littering the working tree and breaking host-side cargo.
RUN mkdir -p /cargo/target /cargo/home/registry /cargo/home/git /cargo/sccache \
    /workspace/playground/node_modules

# Run as a non-root user so files written into the /workspace bind mount
# (generated artefacts, wasm pkg/, mdbook output, node_modules) are owned
# by the host developer, not root. UID/GID default to the conventional
# first-user 1000; override with `--build-arg UID=$(id -u) --build-arg
# GID=$(id -g)` on hosts that differ. Debian bookworm's base leaves 1000
# free, so the create is unconditional (and fails loudly if it ever isn't).
# The cache dirs at /cargo/* and the playground node_modules mountpoint are
# chowned so a *fresh* named volume initialises dev-owned. CI flips the
# runtime UID back to root via `user:` in docker-compose.yml (AFM_UID=0):
# the ephemeral runner's checkout is owned by a different UID and ownership
# is throwaway there, so root sidesteps cross-UID write failures.
ARG UID=1000
ARG GID=1000
RUN groupadd --gid "${GID}" dev \
    && useradd --uid "${UID}" --gid "${GID}" --create-home --shell /bin/bash dev \
    && chown -R "${UID}:${GID}" /cargo /workspace
ENV HOME=/home/dev

WORKDIR /workspace
USER dev

# Default shell friendly for interactive dev sessions
CMD ["bash"]

########################################################################
# Stage: fuzz — dev superset adding nightly + cargo-fuzz + cargo-udeps
#
# Only `just udeps` / `just fuzz*` / `just coverage-branch` need nightly.
# Splitting them here means a plain `target: dev` image (used by the
# `dev` and `playground` compose services) skips a 90 s + 400 MB
# nightly install. ADR-0002 is preserved because everything still runs
# in a container; `strict-code`'s ban on nightly in `rust-toolchain.toml`
# is preserved because nightly is only an opt-in `cargo +nightly` here.
########################################################################
FROM dev AS fuzz

# The dev stage ends as USER dev; the nightly toolchain + cargo-fuzz/udeps
# installs below write to /usr/local (root-owned), so switch back to root
# for the install layers and drop to dev again at the end.
USER root

# `rustup toolchain install` tries to self-update by looking for the
# rustup binary at $CARGO_HOME/bin/rustup. The inherited
# `CARGO_HOME=/cargo/home` (set in the dev stage for runtime
# volume mounts) is empty at image-build time, so the self-update step
# bails with "rustup is not installed at '/cargo/home'". Override
# the env for this one RUN so rustup finds itself at the parent rust
# image's `/usr/local/cargo` location; the runtime CARGO_HOME setting
# is unaffected.
RUN CARGO_HOME=/usr/local/cargo \
    rustup toolchain install nightly --component rust-src --profile minimal

RUN cargo binstall --no-confirm --locked --root /usr/local \
        cargo-fuzz \
        cargo-udeps

USER dev

########################################################################
# Stage: ci — fuzz superset; the published GHCR image (used by CI matrix
# jobs) carries every tool every recipe might invoke.
########################################################################
FROM fuzz AS ci

########################################################################
# Stage: book — lean image for mdbook build / serve
########################################################################
FROM node-base AS book

COPY --from=cargo-tools /usr/local/bin/mdbook /usr/local/bin/mdbook
COPY --from=cargo-tools /usr/local/bin/mdbook-linkcheck /usr/local/bin/mdbook-linkcheck

# Match the dev stage's non-root user so mdbook output written into the
# /workspace bind mount is host-owned, not root. `book` is FROM node-base
# (not dev), so it creates its own identical `dev` user.
ARG UID=1000
ARG GID=1000
RUN groupadd --gid "${GID}" dev \
    && useradd --uid "${UID}" --gid "${GID}" --create-home --shell /bin/bash dev
ENV HOME=/home/dev

WORKDIR /workspace/crates/afm-book
USER dev
EXPOSE 3000
CMD ["mdbook", "serve", "--hostname", "0.0.0.0", "--port", "3000"]

########################################################################
# Stage: browser — Playwright with Chromium + WebKit for M3 onward
########################################################################
FROM mcr.microsoft.com/playwright:v1.59.1-jammy AS browser

WORKDIR /workspace
CMD ["bash"]
