//! Philox4x64-10 counter-based PRNG (D.E. Shaw Research / Random123).
//!
//! Produces 256 bits of pseudorandom output per call from a 128-bit key
//! and 256-bit counter. Matches the device implementation in
//! `cuda/vanity_kernel.cu` exactly so the host can reproduce GPU output
//! for verification.
//!
//! Used here as a fast PRG to drive Ed25519 vanity key search. Output
//! is consumed only via clamp + scalar multiplication (one-way), so the
//! lack of formal CSPRNG status is acceptable; security floor is the
//! 128-bit key from `OsRng`.

const M0: u64 = 0xD2E7470EE14C6C93;
const M1: u64 = 0xCA5A826395121157;
const W0: u64 = 0x9E3779B97F4A7C15;
const W1: u64 = 0xBB67AE8584CAA73B;

pub fn philox4x64_10(mut ctr: [u64; 4], mut key: [u64; 2]) -> [u64; 4] {
    for r in 0..10 {
        if r > 0 {
            key[0] = key[0].wrapping_add(W0);
            key[1] = key[1].wrapping_add(W1);
        }
        let prod0 = (M0 as u128).wrapping_mul(ctr[0] as u128);
        let prod1 = (M1 as u128).wrapping_mul(ctr[2] as u128);
        let hi0 = (prod0 >> 64) as u64;
        let lo0 = prod0 as u64;
        let hi1 = (prod1 >> 64) as u64;
        let lo1 = prod1 as u64;
        ctr = [
            hi1 ^ ctr[1] ^ key[0],
            lo1,
            hi0 ^ ctr[3] ^ key[1],
            lo0,
        ];
    }
    ctr
}

/// Produces 32 little-endian bytes from one Philox4x64-10 call.
/// Matches `philox_block32` in the CUDA kernel.
pub fn philox_block32(k0: u64, k1: u64, idx: u64, side: u64) -> [u8; 32] {
    let out = philox4x64_10([idx, side, 0, 0], [k0, k1]);
    let mut bytes = [0u8; 32];
    for i in 0..4 {
        bytes[i * 8..i * 8 + 8].copy_from_slice(&out[i].to_le_bytes());
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    // Reference vectors from Random123 tests/kat_vectors.
    #[test]
    fn kat_zero() {
        let out = philox4x64_10([0, 0, 0, 0], [0, 0]);
        assert_eq!(
            out,
            [
                0x16554d9eca36314c,
                0xdb20fe9d672d0fdc,
                0xd7e772cee186176b,
                0x7e68b68aec7ba23b,
            ]
        );
    }

    #[test]
    fn kat_pi_e() {
        // ctr = digits of pi, key = digits of e
        let out = philox4x64_10(
            [
                0x243f6a8885a308d3,
                0x13198a2e03707344,
                0xa4093822299f31d0,
                0x082efa98ec4e6c89,
            ],
            [0x452821e638d01377, 0xbe5466cf34e90c6c],
        );
        assert_eq!(
            out,
            [
                0xa528f45403e61d95,
                0x38c72dbd566e9788,
                0xa5a1610e72fd18b5,
                0x57bd43b5e52b7fe6,
            ]
        );
    }

    #[test]
    fn block32_deterministic() {
        let a = philox_block32(1, 2, 42, 0);
        let b = philox_block32(1, 2, 42, 0);
        assert_eq!(a, b);
    }

    #[test]
    fn block32_distinct_sides() {
        let a = philox_block32(1, 2, 42, 0);
        let b = philox_block32(1, 2, 42, 1);
        assert_ne!(a, b);
    }
}
