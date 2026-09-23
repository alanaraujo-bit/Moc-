//! Operating-system randomness (BCryptGenRandom on Windows).

use rand::rngs::OsRng;
use rand::RngCore;

pub fn bytes<const N: usize>() -> [u8; N] {
    let mut out = [0u8; N];
    OsRng.fill_bytes(&mut out);
    out
}

pub fn fill(buf: &mut [u8]) {
    OsRng.fill_bytes(buf);
}

/// Uniform integer in `0..bound` without modulo bias (rejection sampling).
pub fn below(bound: u32) -> u32 {
    assert!(bound > 0, "bound must be positive");
    // Largest multiple of `bound` that fits in u32; reject values above it.
    let zone = u32::MAX - (u32::MAX % bound);
    loop {
        let v = OsRng.next_u32();
        if v < zone {
            return v % bound;
        }
    }
}

/// Fisher–Yates shuffle driven by the OS RNG.
pub fn shuffle<T>(items: &mut [T]) {
    for i in (1..items.len()).rev() {
        let j = below((i + 1) as u32) as usize;
        items.swap(i, j);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn below_stays_in_range_and_covers_it() {
        let mut seen = [false; 7];
        for _ in 0..2_000 {
            let v = below(7) as usize;
            assert!(v < 7);
            seen[v] = true;
        }
        assert!(seen.iter().all(|s| *s));
    }

    #[test]
    fn bytes_are_not_constant() {
        assert_ne!(bytes::<32>(), bytes::<32>());
    }
}
