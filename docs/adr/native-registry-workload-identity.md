---
date: 2026-10-21
status: accepted
---

# Use native registry workload identity

This decision supersedes the token-selection portion of [ADR 0015](0015-crates-io-publication-and-semver.md).
The package selection, dependency order, semver checks, and publication permission remain as recorded there.

The publication workflow uses crates.io Trusted Publishing for every selected package.
It has no fixed-token mode or fallback to a GitHub secret.
An expired or revoked key is a retirement candidate and provides no reason to issue a replacement.

Each selected package needs an existing crates.io identity and the exact repository, workflow, and environment binding.
An unpublished package or missing registration stays unavailable until the owner approves its separate setup.
The workflow keeps its default dry run and never performs a bootstrap upload to test authentication.

Validate workflow syntax and the existing non-publishing package checks before deployment.
Registry authentication remains unverified until the intended package's source-bound identity exchange succeeds.
See [credential delivery](../credential-delivery.md) for the current authentication boundary.
