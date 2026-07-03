//! A fast, dependency-free hasher for transposition-table keys.
//!
//! The search's transposition tables are keyed by `(Mask, Mask)` = two `u64`s
//! (the black/white bitboards). `std::collections::HashMap`'s default
//! `RandomState` (SipHash) is cryptographically strong but slow, and that hashing
//! cost dominates hot-loop TT probes. `FxHasher` is an `FxHash`-style multiply-xor
//! hasher: for our fixed 16-byte key it is a couple of `wrapping_mul`s, which is
//! far cheaper while spreading bits well enough for a search table.
//!
//! Speed vs the default is verified by `benches/hash.rs`.

use std::hash::{BuildHasher, Hasher};

// Odd multiplier with good avalanche behaviour (the constant used by rustc's
// FxHash / the `fxhash` crate).
const K: u64 = 0x51_7c_c1_b7_27_22_0a_95;

/// Multiply-xor hasher. Cheap for the small, fixed-width integer keys used by the
/// transposition tables. Not suitable for untrusted / adversarial input.
#[derive(Default)]
pub struct FxHasher {
    hash: u64,
}

impl FxHasher {
    #[inline]
    fn add(&mut self, i: u64) {
        // Rotate so successive words don't cancel, xor in the word, then mix.
        self.hash = (self.hash.rotate_left(5) ^ i).wrapping_mul(K);
    }
}

impl Hasher for FxHasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        // Fallback path: fold byte-wise. The hot path is `write_u64` below.
        for &b in bytes {
            self.add(b as u64);
        }
    }

    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.add(i);
    }

    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.add(i as u64);
    }

    #[inline]
    fn write_usize(&mut self, i: usize) {
        self.add(i as u64);
    }

    #[inline]
    fn finish(&self) -> u64 {
        self.hash
    }
}

/// `BuildHasher` for [`FxHasher`], usable as the third type parameter of
/// `std::collections::HashMap`.
#[derive(Default, Clone)]
pub struct FxBuildHasher;

impl BuildHasher for FxBuildHasher {
    type Hasher = FxHasher;
    #[inline]
    fn build_hasher(&self) -> FxHasher {
        FxHasher::default()
    }
}
