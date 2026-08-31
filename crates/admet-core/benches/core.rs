//! Criterion micro-benchmarks for `admet-core`.
//!
//! Manual chapter 24. Run with `just bench`, or `cargo bench -p admet-core`.
//!
//! # Why this file exists now, before the code it benchmarks
//!
//! Manual ch. 24.1: **establish a baseline before changing anything.** Without a
//! baseline, "faster" is an opinion. Having the harness in place from the scaffold
//! means the first measurement happens the day the parser lands, not the week
//! before submission when there is nothing to compare against.
//!
//! # What to actually do with it
//!
//! The batch-size sweep below is a **free figure** for the performance chapter.
//! Plotting throughput at five batch sizes gives you a real curve with a visible
//! knee, which is empirical justification for choosing 64 rather than an assumed
//! constant. Examiners notice the difference between a number you chose and a
//! number you measured.
//!
//! Criterion reports confidence intervals and detects regressions between runs, so
//! the output is defensible rather than a single timing that might be noise.
//!
//! Benchmarks of unimplemented functions are commented out rather than deleted:
//! uncomment each as its increment lands.

use criterion::{black_box, criterion_group, criterion_main, Criterion};

use admet_core::fingerprint::{can_exceed, Fingerprint, ReferenceSet};
use admet_core::triage::{top_k, Scored};
use admet_core::validate_input;

/// Deterministic pseudo-random fingerprint.
///
/// A small LCG rather than the `rand` crate: benchmarks must be reproducible run
/// to run, and pulling a dependency in for 200 bits of noise is not worth it.
fn synthetic_fingerprint(seed: u64, bits_set: usize) -> Fingerprint {
    let mut fp = Fingerprint::new();
    let mut state = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
    for _ in 0..bits_set {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        fp.set((state >> 33) as usize);
    }
    fp
}

/// Input validation. Should be nanoseconds — it is the first thing every request
/// touches, and the cheapest possible rejection of garbage.
fn bench_validation(c: &mut Criterion) {
    let aspirin = "CC(=O)Oc1ccccc1C(=O)O";
    c.bench_function("validate_input/aspirin", |b| {
        b.iter(|| validate_input(black_box(aspirin)))
    });
}

/// Tanimoto similarity.
///
/// The number to look for is roughly **40 ns**. If it is much higher, the
/// `[u64; 32]` bitset is not lowering to hardware `POPCNT` — check that the build
/// is `--release` before concluding anything about the code.
fn bench_tanimoto(c: &mut Criterion) {
    let a = synthetic_fingerprint(1, 60);
    let b_fp = synthetic_fingerprint(2, 60);

    let mut group = c.benchmark_group("fingerprint");
    group.bench_function("tanimoto", |b| {
        b.iter(|| black_box(&a).tanimoto(black_box(&b_fp)))
    });
    group.bench_function("popcount", |b| b.iter(|| black_box(&a).popcount()));
    // Target: ~2 ns, i.e. roughly 20x cheaper than the comparison it avoids.
    group.bench_function("can_exceed", |b| {
        b.iter(|| can_exceed(black_box(60), black_box(200), black_box(0.4)))
    });
    group.finish();
}

/// Nearest-neighbour search across a reference set.
///
/// The applicability-domain check on every prediction, so it sits directly in the
/// latency budget for NFR-01. The sweep over set sizes shows whether the popcount
/// prune is doing its job: growth should be visibly sub-linear.
fn bench_domain_search(c: &mut Criterion) {
    let query = synthetic_fingerprint(0, 60);

    let mut group = c.benchmark_group("applicability_domain");
    for size in [100usize, 1_000, 10_000, 25_000] {
        let refs = ReferenceSet::from_fingerprints(
            (0..size as u64)
                .map(|i| synthetic_fingerprint(i + 10, 40 + (i % 80) as usize))
                .collect(),
        );
        group.throughput(criterion::Throughput::Elements(size as u64));
        group.bench_with_input(format!("assess_{size}"), &refs, |b, refs| {
            b.iter(|| refs.assess(black_box(&query)))
        });
    }
    group.finish();
}

/// Bounded-heap top-k.
///
/// The sweep over `n` at fixed `k` is the evidence for the `O(n log k)` claim in
/// the complexity table: doubling `n` should roughly double the time, with `k`
/// barely mattering.
fn bench_top_k(c: &mut Criterion) {
    let mut group = c.benchmark_group("triage");
    for n in [1_000usize, 10_000, 50_000] {
        let items: Vec<Scored> = (0..n)
            .map(|i| Scored {
                inchikey: format!("KEY{i:010}XXXXXXXXXX-N"),
                // Deliberately not monotonic: an ascending stream is the best
                // case for the heap (every item beats the root) and an
                // descending one is the worst. Interleaving is realistic.
                score: ((i * 7919) % n) as f32 / n as f32,
                row_index: i,
            })
            .collect();

        group.throughput(criterion::Throughput::Elements(n as u64));
        group.bench_with_input(format!("top_100_of_{n}"), &items, |b, items| {
            b.iter(|| top_k(black_box(items).iter().cloned(), 100))
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
//  Uncomment as each increment lands.
// ---------------------------------------------------------------------------
//
// /// Increment 2. Target ~2 us for aspirin. This is the O(L) single-pass claim.
// fn bench_parse(c: &mut Criterion) {
//     let mut group = c.benchmark_group("parse");
//     for (name, smiles) in [
//         ("ethanol", "CCO"),
//         ("aspirin", "CC(=O)Oc1ccccc1C(=O)O"),
//         ("caffeine", "Cn1cnc2c1c(=O)[nH]c(=O)n2C"),
//         ("atorvastatin", "CC(C)c1c(C(=O)Nc2ccccc2)c(-c2ccccc2)c(-c2ccc(F)cc2)n1CC[C@@H](O)C[C@@H](O)CC(=O)O"),
//     ] {
//         group.throughput(criterion::Throughput::Bytes(smiles.len() as u64));
//         group.bench_function(name, |b| {
//             b.iter(|| admet_core::smiles::parse(black_box(smiles)).unwrap())
//         });
//     }
//     group.finish();
// }
//
// /// Increment 2. Featurisation is O(N^2) in the dense adjacency, so this is the
// /// benchmark that shows whether the N=128 cap is actually cheap in practice --
// /// the claim ADR-03 rests on.
// fn bench_featurise(c: &mut Criterion) {
//     let g = admet_core::smiles::parse("CC(=O)Oc1ccccc1C(=O)O").unwrap();
//     c.bench_function("featurise/aspirin", |b| {
//         b.iter(|| admet_core::features::featurise(black_box(&g)).unwrap())
//     });
// }
//
// /// Increment 2. Target ~8 us. Runs on every cache miss, so it is on the
// /// latency path for NFR-01.
// fn bench_canonicalise(c: &mut Criterion) {
//     let g = admet_core::smiles::parse("CC(=O)Oc1ccccc1C(=O)O").unwrap();
//     c.bench_function("morgan_labels/aspirin", |b| {
//         b.iter(|| admet_core::canonical::morgan_labels(black_box(&g)).unwrap())
//     });
// }

criterion_group!(
    benches,
    bench_validation,
    bench_tanimoto,
    bench_domain_search,
    bench_top_k,
);
criterion_main!(benches);
