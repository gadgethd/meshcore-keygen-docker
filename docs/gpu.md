# GPU Acceleration

## Performance

Measured throughput on real hardware:

| | Throughput | Hardware |
|-|------------|----------|
| **CPU** | ~1.3M keys/sec | AMD Ryzen 9 7950X3D (32 threads) |
| **GPU** | ~68M keys/sec | NVIDIA RTX 4090 (24GB) |

The GPU is roughly **50x faster** than the CPU on this hardware. The speedup comes from running the full Ed25519 pipeline (Philox4x64-10 random bytes, scalar multiply, point compress, prefix check) across thousands of GPU threads in parallel.

## Building with CUDA

```bash
cargo build --release --features cuda
```

Requires the NVIDIA CUDA Toolkit to be installed. The CUDA kernel is compiled at runtime via NVRTC, so `nvrtc` and `cuda` shared libraries must be on PATH.

The `cudarc` dependency is configured for CUDA 13.1 by default. To target a different CUDA version, change the `cuda-13010` feature in `Cargo.toml` to match your installation (e.g. `cuda-12060` for CUDA 12.6).

## How GPU mode works

When `--gpu-only` (or hybrid) is selected, the entire keygen pipeline (Philox4x64-10, scalar clamping, Ed25519 scalar multiplication, point compression, prefix check) runs on the GPU. A single host thread loops: launch kernel batch, sync, check for match, repeat. The TUI progress display works identically for both CPU and GPU modes.

Each `GpuSearcher` draws a 128-bit Philox key from the OS CSPRNG at construction time. The kernel computes `Philox4x64-10(key, counter=tid·iters+iter)` to derive 64 bytes of pseudorandom output per candidate, splits them into the clamped scalar and the signing-nonce prefix, then runs the standard Ed25519 base-point multiplication. The PRNG output is hidden behind discrete-log before any value is exposed, so a 128-bit key (matching Curve25519's DLog floor) provides the relevant security margin without paying for a full CSPRNG like ChaCha.

The Ed25519 field arithmetic uses the ref10 implementation (radix-2^25.5, int32[10] limbs) from [solana-perf-libs](https://github.com/solana-labs/solana-perf-libs), which is a CUDA adaptation of the [SUPERCOP](https://bench.cr.yp.to/supercop.html) reference code. Scalar multiplication uses windowed lookup tables with 32 precomputed basepoint multiples.

## Verification

Use `--verify` to cross-check GPU-generated keys against the CPU implementation. This is useful for validating correctness after modifying the CUDA kernel.

## Sources

- [solana-perf-libs](https://github.com/solana-labs/solana-perf-libs) — CUDA Ed25519 field arithmetic (ref10)
- [cudarc](https://github.com/coreylowman/cudarc) — safe Rust bindings for the CUDA driver API
