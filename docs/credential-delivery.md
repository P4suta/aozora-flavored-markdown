# Credential delivery

The `publish-crates.yml` workflow uses native crates.io Trusted Publishing in the protected `release` environment.
It accepts only the short-lived token from the official registry authentication action.
It has no `CARGO_TOKEN` fallback or `use_oidc` selector.
The default dry run performs no upload and requests no registry token.

Configure the exact `P4suta/aozora-flavored-markdown`, `publish-crates.yml`, and `release` binding for each selected package.
The selection includes `aozora-flavored-markdown`, `aozora-flavored-markdown-cli`, `aozora-flavored-markdown-epub`, and `aozora-flavored-markdown-epub-cli` after applying the existing skip list.
Package existence and trusted-publisher registration need their own current evidence.
An unpublished package requires separate authorized setup; this migration never issues a replacement fixed key or publishes to create that package.

After the reviewed workflow reaches the default branch, verify its exact source revision and intended registry binding before retiring the old Doppler sync and GitHub copy.
Keep authentication, package verification, and publication permission distinct.
See [the current decision](adr/native-registry-workload-identity.md) for the change from the earlier token-backed bootstrap path.
