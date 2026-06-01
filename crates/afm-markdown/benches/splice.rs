//! afm glue-cost throughput sentinel.
//!
//! Measures the cost that is *afm's own* — comrak-wrap plus the
//! AST-level Aozora sentinel splice (`ast_splice::splice_into_ast`) —
//! rather than the hot 青空文庫 parse, which lives in the git-pinned
//! `aozora` workspace and is benched there (`aozora-pipeline`'s
//! `tokenize_compare` / `classify_kaeriten`).
//!
//! Both arms call the public [`afm_markdown::render_to_string`] on the
//! *same* afm source. They differ only in `Options`:
//!
//! * `afm` arm — [`Options::afm_default`]: the lexer pre-pass runs, the
//!   PUA sentinels are spliced into the comrak AST, and the brand
//!   rewrite (`aozora-` → `afm-`) fires. This is the full afm path.
//! * `comrak_only` arm — [`Options::gfm_only`]: `aozora_enabled = false`,
//!   so the input flows straight through vanilla comrak with no splice.
//!
//! The delta between the two arms is the afm-specific overhead in
//! isolation: a regression in the splice walk or the comrak-format
//! finalisation shows up as a widening gap without the aozora parse
//! cost (shared by both arms via the lexer pre-pass… which the
//! `comrak_only` arm skips entirely, so the `afm` arm additionally
//! carries the lexer — see the module note). The `comrak_only` arm is
//! the floor; the `afm` arm is floor + lexer + splice.
//!
//! Input bands mirror the corpus distribution: `prose` is sentinel-free
//! Markdown (the splice walk finds nothing and must stay cheap),
//! `mixed` interleaves ruby / bouten / annotation triggers at the
//! corpus-median density, and `dense` is annotation-heavy so the
//! splice walk does maximal work.

use std::hint::black_box;

use afm_markdown::{Options, render_to_string};
use criterion::{Criterion, Throughput, criterion_group, criterion_main};

/// Sentinel-free GFM prose — the splice walk traverses the AST and
/// finds nothing to rewrite. Pins the "afm overhead on plain Markdown"
/// floor.
fn build_prose(target: usize) -> String {
    let unit = "The quick brown fox **jumps** over the _lazy_ dog.\n\n";
    let cycles = target.div_ceil(unit.len());
    unit.repeat(cycles)
}

/// Corpus-median density: roughly one Aozora construct per paragraph,
/// interleaved with plain GFM prose.
fn build_mixed(target: usize) -> String {
    let unit = "｜青梅《おうめ》の段落。**強調**もあり、［＃改ページ］を挟む。\n\n";
    let cycles = target.div_ceil(unit.len());
    unit.repeat(cycles)
}

/// Annotation-heavy pathological band: every line carries a ruby span,
/// so the splice walk rewrites a sentinel on (almost) every inline.
fn build_dense(target: usize) -> String {
    let unit = "青梅《おうめ》梅田《うめだ》大阪《おおさか》\n";
    let cycles = target.div_ceil(unit.len());
    unit.repeat(cycles)
}

fn bench_splice(c: &mut Criterion) {
    // ~64 KiB per band — large enough to dominate per-call fixed costs
    // (arena setup, comrak document scaffolding) without making the
    // bench wall-clock noticeable in the `just bench` smoke run.
    const SIZE: usize = 64 * 1024;
    let prose = build_prose(SIZE);
    let mixed = build_mixed(SIZE);
    let dense = build_dense(SIZE);

    let afm = Options::afm_default();
    let comrak_only = Options::gfm_only();

    for (label, sample) in [("prose", &prose), ("mixed", &mixed), ("dense", &dense)] {
        let mut g = c.benchmark_group(label);
        g.throughput(Throughput::Bytes(sample.len() as u64));

        // Full afm path: lexer pre-pass + comrak + AST splice + brand
        // rewrite. This is the number that regresses when afm's own
        // glue gets slower.
        g.bench_function("afm", |b| {
            b.iter(|| {
                let out = render_to_string(black_box(sample.as_str()), &afm);
                black_box(out);
            });
        });

        // Floor: vanilla comrak, no aozora pass. Subtract this arm from
        // the `afm` arm to read the afm-specific overhead.
        g.bench_function("comrak_only", |b| {
            b.iter(|| {
                let out = render_to_string(black_box(sample.as_str()), &comrak_only);
                black_box(out);
            });
        });

        g.finish();
    }
}

criterion_group!(splice_benches, bench_splice);
criterion_main!(splice_benches);
