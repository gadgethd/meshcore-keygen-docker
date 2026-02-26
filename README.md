# mc-keygen

Vanity Ed25519 key generator for [MeshCore](https://github.com/ripplebiz/MeshCore). Find keys whose public key starts with a chosen hex prefix.

## Usage

```
mc-keygen <PREFIX> [OPTIONS]
```

**Options:**
- `-t, --threads <N>` — worker threads (default: all cores)
- `--json` — output result as JSON (no TUI, no color)

**Examples:**
```bash
mc-keygen AB           # find a key starting with AB
mc-keygen DEAD -t 4    # use 4 threads
mc-keygen AB --json    # machine-readable output
```

Prefix length determines difficulty — each hex char multiplies expected attempts by 16:

| Prefix | Expected attempts |
|--------|-------------------|
| 1 char | 16                |
| 2 char | 256               |
| 3 char | 4,096             |
| 4 char | 65,536            |
| 5 char | ~1M               |
| 6 char | ~16M              |

## Building

```bash
cargo build --release
```

## How it works

1. Generate random 32-byte seed
2. SHA-512 hash, clamp scalar (per Ed25519 spec), derive public key
3. Check if public key hex starts with the target prefix
4. Repeat across threads until a match is found

Keys starting with `00` or `FF` are skipped (reserved by MeshCore).

## Sources

- [MeshCore](https://github.com/ripplebiz/MeshCore) — the mesh networking firmware these keys are for
- [MeshCore mc-keygen web tool](https://github.com/ripplebiz/MeshCore/blob/main/variants/MeshCore_nRF52_USB_Dongle/companion_app/mc-keygen.html) — reference implementation of the key generation algorithm
- [Ed25519 / RFC 8032](https://datatracker.ietf.org/doc/html/rfc8032) — the signature scheme spec
- [curve25519-dalek](https://github.com/dalek-cryptography/curve25519-dalek) — Rust Ed25519 elliptic curve library
- [ratatui](https://github.com/ratatui/ratatui) — TUI framework used for the progress display
