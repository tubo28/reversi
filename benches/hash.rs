//! Micro-benchmark confirming the custom `FxBuildHasher` is faster than the
//! default `RandomState` (SipHash) for the transposition-table workload:
//! `HashMap<(u64, u64), i32>` keyed by board bitmasks.
//!
//! Run with: `cargo bench --bench hash`
//! (declared with `harness = false`, so this is a plain `main`).

use std::collections::hash_map::RandomState;
use std::collections::HashMap;
use std::hash::BuildHasher;
use std::hint::black_box;
use std::time::Instant;

use reversi::reversi::hash::FxBuildHasher;
use reversi::reversi::rand::Xor128;

// A full 64-bit value out of the 31-bit generator.
fn rand_u64(rng: &mut Xor128) -> u64 {
    let a = rng.next() as u64;
    let b = rng.next() as u64;
    let c = rng.next() as u64;
    (a << 33) ^ (b << 17) ^ c
}

// Build `n` transposition-table-shaped keys: a `(black, white)` pair of disjoint
// bitboards, mimicking the real key distribution used by the search TT.
fn make_keys(n: usize) -> Vec<(u64, u64)> {
    let mut rng = Xor128::from_seed(12345);
    (0..n)
        .map(|_| {
            let black = rand_u64(&mut rng);
            let white = rand_u64(&mut rng) & !black;
            (black, white)
        })
        .collect()
}

// Fill a HashMap with the given hasher, then probe every key `probe_passes`
// times. Returns (elapsed, checksum) where the checksum defeats dead-code
// elimination. Repeated `rounds` times; the fastest round is reported.
fn bench<S: BuildHasher + Default + Clone>(
    keys: &[(u64, u64)],
    probe_passes: usize,
    rounds: usize,
) -> (f64, u64) {
    let mut best = f64::INFINITY;
    let mut checksum = 0u64;
    for _ in 0..rounds {
        let start = Instant::now();

        let mut map: HashMap<(u64, u64), i32, S> = HashMap::default();
        for (i, &k) in keys.iter().enumerate() {
            map.insert(black_box(k), i as i32);
        }

        let mut acc = 0i64;
        for _ in 0..probe_passes {
            for &k in keys {
                if let Some(&v) = map.get(&black_box(k)) {
                    acc = acc.wrapping_add(v as i64);
                }
            }
        }

        let elapsed = start.elapsed().as_secs_f64();
        checksum = checksum.wrapping_add(acc as u64);
        best = best.min(elapsed);
    }
    (best, checksum)
}

fn main() {
    const N: usize = 200_000; // keys inserted
    const PROBE_PASSES: usize = 8; // full lookup sweeps
    const ROUNDS: usize = 5; // repeat, keep fastest

    let keys = make_keys(N);
    let ops = (N + N * PROBE_PASSES) as f64; // inserts + probes

    // Warm up caches / allocator so the first hasher isn't unfairly penalised.
    black_box(bench::<RandomState>(&keys, 1, 1));
    black_box(bench::<FxBuildHasher>(&keys, 1, 1));

    let (sip_t, c1) = bench::<RandomState>(&keys, PROBE_PASSES, ROUNDS);
    let (fx_t, c2) = bench::<FxBuildHasher>(&keys, PROBE_PASSES, ROUNDS);
    black_box((c1, c2));

    let sip_ns = sip_t / ops * 1e9;
    let fx_ns = fx_t / ops * 1e9;

    println!("transposition-table hash benchmark");
    println!("  keys = {N}, probe passes = {PROBE_PASSES}, rounds = {ROUNDS} (best shown)");
    println!("  {:<16} {:>10.3} ms   {:>7.3} ns/op", "SipHash (std)", sip_t * 1e3, sip_ns);
    println!("  {:<16} {:>10.3} ms   {:>7.3} ns/op", "FxHasher", fx_t * 1e3, fx_ns);
    println!("  speedup: {:.2}x", sip_t / fx_t);

    if fx_t < sip_t {
        println!("  => FxHasher is faster. OK");
    } else {
        println!("  => WARNING: FxHasher is NOT faster; revisit the hash mixing.");
    }
}
