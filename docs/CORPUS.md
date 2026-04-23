# Corpus setup for parser sweep

This guide explains how to feed real-world aozora-format text to the afm
parser via the `just corpus-sweep` target. The sweep is **opt-in**: it
requires you to point the `AFM_CORPUS_ROOT` environment variable at a
directory on your machine. Without that, `just corpus-sweep` prints a
usage hint and does nothing (zero-exit), so you can run `just test` and
CI jobs without a corpus configured and see only green.

See `docs/adr/0007-corpus-sweep-strategy.md` for the design rationale;
this file is purely operational.

## What you need

A directory containing `.txt` files in Shift_JIS encoding (the Aozora
Bunko native format). The directory structure does not matter — the
sweep walks recursively and picks up every `.txt` it finds. Labels in
diagnostics are paths relative to the root, so a structured layout
makes failure messages easier to read, but a flat dump works too.

## Option A: `aozorahack/aozorabunko_text` ZIP

The most convenient source of bulk text is the community mirror at
<https://github.com/aozorahack/aozorabunko_text>. It contains
~13 750 works (~1 010 authors) in a stable `cards/<id>/files/<id>_*/<id>_*.txt`
layout. A snapshot ZIP is linked from the repo's README.

Set-up on WSL:

```bash
# 1. Extract to a location outside the afm repo. Avoid the Windows NTFS
#    mount — it's much slower than ext4 for the ~36 000 small files.
mkdir -p ~/aozora-corpus
unzip /mnt/c/Users/<you>/Downloads/aozorabunko_text-master.zip \
    -d ~/aozora-corpus

# 2. Point AFM_CORPUS_ROOT at the extracted directory. The ZIP unpacks
#    to an `aozorabunko_text-master/` subdirectory; target that.
export AFM_CORPUS_ROOT=$HOME/aozora-corpus/aozorabunko_text-master

# 3. Run the sweep.
just corpus-sweep
```

Expected output shape:

```text
corpus sweep: provenance = filesystem:/corpus
corpus sweep summary: 13748 passed, 0 panics, N with leaked ［＃ markers, M decode errors, 0 I/O skips
  first leaked-marker samples: [...]
  first decode-error samples: [...]
```

Leaked-marker samples point to files whose text contains `［＃` annotation
shapes the parser hasn't yet classified — these are useful diagnostic
pointers for parser development, not hard test failures (I2 is
report-only until ADR-0005's paired-container hook lands).

## Option B: shallow `git clone`

If you prefer a live clone (slightly newer than the snapshot ZIP and
pullable):

```bash
git clone --depth 1 https://github.com/aozorahack/aozorabunko_text \
    ~/aozora-corpus
export AFM_CORPUS_ROOT=$HOME/aozora-corpus
just corpus-sweep
```

Same layout regularity applies; `FilesystemCorpus` reads both forms
identically.

## Option C: your own curation

Any directory of `.txt` files works. Curate a small subset when you
want fast feedback, or point at a completely unrelated aozora-format
corpus you happen to have. The sweep has no preferences about content
identity; it only cares that the bytes decode as SJIS and that the
parser survives them.

```bash
mkdir -p /tmp/my-corpus
cp path/to/work1.txt path/to/work2.txt /tmp/my-corpus/
export AFM_CORPUS_ROOT=/tmp/my-corpus
just corpus-sweep
```

## What the sweep checks

Invariants enforced / reported by the sweep harness are defined in the
test file itself (`crates/afm-parser/tests/corpus_sweep.rs`). As of the
initial M2-S3 landing:

| Invariant | Semantic | Gate |
|---|---|---|
| I1 | Parser never panics | **Hard** — test fails listing offending labels |
| I2 | No `［＃` markers leak through rendered HTML | Report-only (see ADR-0007) |
| I5 | SJIS decode succeeds | Report-only |

I3 (round-trip stability) and I4 (HTML well-formedness) land in
M2-S6 and M2-S4 respectively.

## Making the env var persistent

If you work with a corpus regularly, put the export in your shell RC
file (or the project-local direnv/chezmoi equivalent). Example with
direnv:

```bash
# .envrc
export AFM_CORPUS_ROOT=$HOME/aozora-corpus/aozorabunko_text-master
```

Or chezmoi-managed `~/.bashrc`:

```bash
[ -d "$HOME/aozora-corpus/aozorabunko_text-master" ] && \
    export AFM_CORPUS_ROOT=$HOME/aozora-corpus/aozorabunko_text-master
```

The guard ensures no error when the corpus directory is absent (e.g.
on a fresh checkout).

## Docker details

`just corpus-sweep` bind-mounts `$AFM_CORPUS_ROOT` into the container
read-only at `/corpus`, then sets the in-container
`AFM_CORPUS_ROOT=/corpus`. The host path doesn't need to be stable —
only the container-side path (`/corpus`) is load-bearing, and it's
handled entirely by the Justfile.

This is why *setting* `AFM_CORPUS_ROOT` in `docker-compose.yml` alone
wouldn't work: the container would have the environment variable but
no access to the files. The bind-mount has to happen at
`docker compose run` invocation time.

## Troubleshooting

**"AFM_CORPUS_ROOT is not set; sweep has nothing to walk."**
Expected when the variable is unset. Not a failure. Export the
variable per the options above.

**"AFM_CORPUS_ROOT=... is not a directory."**
The path doesn't exist or points at a file. Re-check the variable
value; likely a typo or the ZIP wasn't extracted.

**Sweep reports "M decode errors".**
Some file in the tree isn't Shift_JIS. The Aozora mirror's `cards/`
subtree is pure SJIS, but sibling files at the root (`README.md`,
`.github/`) are UTF-8. If the decode-error samples all look like
non-corpus files, narrow `AFM_CORPUS_ROOT` to the `cards/` subtree:

```bash
export AFM_CORPUS_ROOT=$HOME/aozora-corpus/aozorabunko_text-master/cards
just corpus-sweep
```

**Sweep reports "N panics".**
This is a real parser bug — the sweep's I1 invariant fires a test
failure. The "first panic samples" line in the summary lists up to
ten offending file labels so you can reproduce in isolation:

```bash
# Copy one offending file out, decode to UTF-8, feed to parser.
cp "$AFM_CORPUS_ROOT/<label>" /tmp/repro.sjis.txt
iconv -f CP932 -t UTF-8 /tmp/repro.sjis.txt > /tmp/repro.utf8.txt
# Then wire into a cargo test, or interactively via `just shell`.
```

**Sweep takes too long.**
Sweep time is ~parser parse time × N. On ~13 000 files and the
current parser, expect several minutes for the full aozorahack mirror
(first run; subsequent runs are bounded by file I/O). Narrow the root
to a subtree or curated selection during iterative work.

## Relationship to golden tests

The sweep and the golden fixture tests are complementary, not
substitutes:

- `just spec-golden-56656` — one work, exact expected HTML, regression
  gate. Hard-fails on any output drift.
- `just corpus-sweep` — arbitrary works, invariant checks only.
  Reports aggregate counts; I1 hard-fails on parser panic.

Tier A (golden) catches "did we break the known-good behaviour for
work X?". Sweep catches "is there any real-world aozora text that
breaks the parser in a way no single test covers?". Run both when a
change might affect parser behaviour across multiple inputs.

## Where the code lives

- `crates/afm-corpus/` — the `CorpusSource` trait and three impls.
- `crates/afm-parser/tests/corpus_sweep.rs` — the sweep harness.
- `Justfile::corpus-sweep` — bind-mount / env bridge.
- `docs/adr/0007-corpus-sweep-strategy.md` — design rationale.
