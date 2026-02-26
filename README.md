# mc-keygen

Vanity Ed25519 key generator for [MeshCore](https://github.com/ripplebiz/MeshCore). Find keys whose public key starts with a chosen hex prefix.

## Usage

```
mc-keygen <PREFIX>... [OPTIONS]
```

**Options:**
- `-t, --threads <N>` — worker threads (default: all cores)
- `--json` — output result as JSON (no TUI, no color)
- `--gpu` — use CUDA GPU acceleration (requires `cuda` feature)
- `--verify` — cross-check GPU keygen against CPU (requires `cuda` feature)

**Examples:**
```bash
mc-keygen AB             # find a key starting with AB
mc-keygen AB CD EF       # find a key matching ANY of these prefixes
mc-keygen DEAD -t 4      # use 4 threads
mc-keygen AB --json      # machine-readable output
mc-keygen ABCDEF --gpu   # use GPU for longer prefixes
mc-keygen AB CD --gpu    # multi-prefix GPU search
```

### Multiple prefixes

You can pass multiple prefixes in a single invocation. Every generated key is checked against all of them, so searching for N prefixes at once is ~N times more efficient than running N separate searches. The JSON output includes a `matched_prefix` field indicating which prefix was found.

Prefix length determines difficulty — each hex char multiplies expected attempts by 16:

| Prefix | Expected attempts | CPU time | GPU time |
|--------|-------------------|----------|----------|
| 1 char | 16                | instant  | instant  |
| 2 char | 256               | instant  | instant  |
| 3 char | 4,096             | instant  | instant  |
| 4 char | 65,536            | instant  | instant  |
| 5 char | ~1M               | <1s      | instant  |
| 6 char | ~16M              | ~12s     | <1s      |
| 7 char | ~268M             | ~3.5m    | ~4s      |
| 8 char | ~4.3B             | ~55m     | ~63s     |

## Performance

Measured throughput on real hardware:

| | Throughput | Hardware |
|-|------------|----------|
| **CPU** | ~1.3M keys/sec | AMD Ryzen 9 7950X3D (32 threads) |
| **GPU** | ~68M keys/sec | NVIDIA RTX 4090 (24GB) |

The GPU is roughly **50x faster** than the CPU on this hardware. The speedup comes from running the full Ed25519 pipeline (SHA-512, scalar multiply, point compress, prefix check) across thousands of GPU threads in parallel.

## Benchmarks

Run with `cargo bench`:

**Prefix matching overhead** — isolated prefix check cost per key:

| Prefixes | Time per check |
|----------|---------------|
| 1        | ~0.5 ns       |
| 5        | ~2.4 ns       |
| 10       | ~4.7 ns       |

**Full keygen + match pipeline** — end-to-end per-key cost (single thread):

| Prefixes | Time per key | Throughput |
|----------|-------------|------------|
| 0 (keygen only) | 12.5 µs | 80K keys/s |
| 1        | 12.5 µs     | 80K keys/s |
| 5        | 12.2 µs     | 82K keys/s |
| 10       | 12.2 µs     | 82K keys/s |
| 20       | 12.2 µs     | 82K keys/s |

Multi-prefix matching adds **no measurable overhead** — the prefix check (~5 ns for 10 prefixes) is dwarfed by the Ed25519 scalar multiplication (~12.5 µs). Measured on AMD Ryzen 9 7950X3D.

## Building

**CPU only:**
```bash
cargo build --release
```

**With CUDA GPU support:**
```bash
cargo build --release --features cuda
```

Requires the NVIDIA CUDA Toolkit to be installed. The CUDA kernel is compiled at runtime via NVRTC, so `nvrtc` and `cuda` shared libraries must be on PATH.

The `cudarc` dependency is configured for CUDA 13.1 by default. To target a different CUDA version, change the `cuda-13010` feature in `Cargo.toml` to match your installation (e.g. `cuda-12060` for CUDA 12.6).

## How it works

1. Generate random 32-byte seed
2. SHA-512 hash, clamp scalar (per Ed25519 spec), derive public key
3. Check if public key hex starts with the target prefix
4. Repeat across all cores (or GPU threads) until a match is found

Keys starting with `00` or `FF` are skipped (reserved by MeshCore).

### GPU mode

When `--gpu` is passed, the entire keygen pipeline (SHA-512, scalar clamping, Ed25519 scalar multiplication, point compression, prefix check) runs on the GPU. A single host thread loops: launch kernel batch, sync, check for match, repeat. The TUI progress display works identically for both CPU and GPU modes.

The Ed25519 field arithmetic uses the ref10 implementation (radix-2^25.5, int32[10] limbs) from [solana-perf-libs](https://github.com/solana-labs/solana-perf-libs), which is a CUDA adaptation of the [SUPERCOP](https://bench.cr.yp.to/supercop.html) reference code. Scalar multiplication uses windowed lookup tables with 32 precomputed basepoint multiples.

## Sources

- [MeshCore](https://github.com/ripplebiz/MeshCore) — the mesh networking firmware these keys are for
- [MeshCore mc-keygen web tool](https://github.com/ripplebiz/MeshCore/blob/main/variants/MeshCore_nRF52_USB_Dongle/companion_app/mc-keygen.html) — reference implementation of the key generation algorithm
- [Ed25519 / RFC 8032](https://datatracker.ietf.org/doc/html/rfc8032) — the signature scheme spec
- [curve25519-dalek](https://github.com/dalek-cryptography/curve25519-dalek) — Rust Ed25519 elliptic curve library
- [solana-perf-libs](https://github.com/solana-labs/solana-perf-libs) — CUDA Ed25519 field arithmetic (ref10)
- [cudarc](https://github.com/coreylowman/cudarc) — safe Rust bindings for the CUDA driver API
- [ratatui](https://github.com/ratatui/ratatui) — TUI framework used for the progress display
