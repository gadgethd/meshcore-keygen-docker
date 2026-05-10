# mc-keygen

Vanity Ed25519 key generator for [MeshCore](https://github.com/ripplebiz/MeshCore) with a web UI, job queue, deterministic pause/resume, benchmarking, and GPU support.

## Quick start (Docker)

### 1. Install NVIDIA Container Toolkit

The container requires the NVIDIA Container Toolkit so Docker can access the GPU.

**Fedora / RHEL / CentOS:**
```bash
curl -s -L https://nvidia.github.io/libnvidia-container/stable/rpm/nvidia-container-toolkit.repo \
  | sudo tee /etc/yum.repos.d/nvidia-container-toolkit.repo

sudo dnf install -y nvidia-container-toolkit
sudo nvidia-ctk runtime configure --runtime=docker
sudo systemctl restart docker
```

**Ubuntu / Debian:**
```bash
curl -fsSL https://nvidia.github.io/libnvidia-container/gpgkey \
  | sudo gpg --dearmor -o /usr/share/keyrings/nvidia-container-toolkit-keyring.gpg

curl -s -L https://nvidia.github.io/libnvidia-container/stable/deb/nvidia-container-toolkit.list \
  | sed 's#deb https://#deb [signed-by=/usr/share/keyrings/nvidia-container-toolkit-keyring.gpg] https://#g' \
  | sudo tee /etc/apt/sources.list.d/nvidia-container-toolkit.list

sudo apt-get update
sudo apt-get install -y nvidia-container-toolkit
sudo nvidia-ctk runtime configure --runtime=docker
sudo systemctl restart docker
```

Verify the toolkit is working before starting the container:
```bash
docker run --rm --gpus all nvidia/cuda:12.6.3-base-ubuntu24.04 nvidia-smi
```

### 2. Start the container

```bash
docker compose up --build
# Open http://localhost:8080
```

## Features

- **Web UI** — Dashboard, New Job with live estimates, Queue, Results, Settings
- **CPU + CUDA GPU** — Hybrid search across CPU threads and NVIDIA GPUs
- **Deterministic pause/resume** — master seed + counter, atomic checkpoint files
- **Live estimates** — Type a hex prefix and see expected time to 50/90/95/99% milestones
- **Benchmark mode** — Measure real keys/sec for accurate estimates
- **Job queue** — Create, pause, resume, duplicate, reorder jobs
- **CPU reservation** — Reserve cores for the web UI (default: 1 core)
- **SQLite persistence** — Jobs, results, benchmarks, settings survive restarts
- **REST API** — Full API for external automation
- **WebSocket** — Live status updates at `/api/ws`

## CLI Usage

```
mc-keygen <PREFIX>... [OPTIONS]
```

**Options:**

| Flag | Description |
|------|-------------|
| `-t, --threads <N>` | Worker threads (default: all cores minus reserved) |
| `--json` | Output result as JSON (no TUI) |
| `--json-progress` | Emit JSON progress lines to stdout |
| `--deterministic` | Use deterministic seed+counter mode |
| `--master-seed <HEX>` | Master seed for deterministic mode (64 hex chars) |
| `--checkpoint <PATH>` | Checkpoint file for save/resume |
| `--checkpoint-interval <N>` | Seconds between checkpoint saves (default: 10) |
| `--resume <PATH>` | Resume from a checkpoint file |
| `--start-counter <N>` | Starting counter for deterministic mode |
| `--worker-id <N>` | Worker ID for multi-worker setups |
| `--workers <N>` | Total workers for chunk allocation |
| `--max-attempts <N>` | Stop after N attempts |
| `--max-runtime <N>` | Stop after N seconds |
| `--benchmark` | Run a benchmark with random prefix |
| `--benchmark-prefix-length <N>` | Benchmark prefix length (default: 6) |
| `--benchmark-timeout <N>` | Benchmark timeout in seconds |
| `--serve` | Start web server (requires `server` feature) |

**GPU flags (with `cuda` feature):**

| Flag | Description |
|------|-------------|
| `--cpu-only` | Force CPU-only search |
| `--gpu-only` | Force GPU-only search |
| `--verify` | Verify GPU keygen against CPU |
| `--device <N>` | Select GPU device |

**Examples:**

```bash
mc-keygen AB                      # find key starting with AB
mc-keygen C0DEBA5ED               # 9-char prefix (with warning)
mc-keygen AB CD EF --json         # multi-prefix, JSON output
mc-keygen AB --deterministic --checkpoint /data/checkpoints/job.json
mc-keygen --benchmark --benchmark-prefix-length 6
mc-keygen --serve                 # start web server at :8080
```

## Web UI Pages

### Dashboard
Live overview: active job, keys/s, attempts, queue length, results count, GPU status, CPU allocation, last benchmark.

### New Job
Type hex prefix(es) to see live time estimates using benchmark data. Create jobs with CPU/GPU backend selection, max attempts/runtime, and notes.

### Queue
View all jobs with status, progress, and controls: start, pause, resume, stop, duplicate, delete.

### Results
Found keys with public addresses. Private keys hidden by default with reveal/copy option.

### Settings
Configure CPU reservation, worker threads, checkpoint interval, default backend, timezone, and secret display policy.

## API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/status` | System status |
| GET | `/api/jobs` | List all jobs |
| POST | `/api/jobs` | Create job |
| GET | `/api/jobs/:id` | Get job |
| PATCH | `/api/jobs/:id` | Update job |
| DELETE | `/api/jobs/:id` | Delete job |
| POST | `/api/jobs/:id/pause` | Pause job |
| POST | `/api/jobs/:id/resume` | Resume job |
| POST | `/api/jobs/:id/stop` | Stop job |
| POST | `/api/jobs/:id/restart` | Restart (new seed) |
| POST | `/api/jobs/:id/duplicate` | Duplicate job |
| GET | `/api/results` | List results |
| DELETE | `/api/results/:id` | Delete result |
| GET | `/api/benchmarks` | List benchmarks |
| POST | `/api/benchmarks` | Create benchmark |
| DELETE | `/api/benchmarks/:id` | Delete benchmark |
| POST | `/api/benchmarks/:id/set-default` | Set as default estimate benchmark |
| GET | `/api/settings` | Get settings |
| PATCH | `/api/settings` | Update settings |
| POST | `/api/estimate` | Get time estimate for prefixes |
| GET | `/api/system/cpu` | CPU info |
| GET | `/api/devices` | Available backends |
| GET | `/api/ws` | WebSocket live updates |

## Deterministic model

```
candidate_seed = SHA-256(master_seed || worker_id || counter)
```

- **Pause** = save checkpoint, stop workers
- **Resume** = load checkpoint, continue same master seed + counter
- **Restart** = new random master seed
- **Duplicate** = copy settings, new random master seed

## Building

```bash
# CPU only
cargo build --release

# With CUDA GPU support
cargo build --release --features cuda

# With web server
cargo build --release --features cuda,server
```

## Docker Compose

```yaml
services:
  meshcore-keygen-ui:
    build: .
    ports:
      - "8080:8080"
    volumes:
      - ./data:/data
    environment:
      - TZ=UTC
      - APP_BIND=0.0.0.0:8080
      - RESERVED_CPU_CORES=1
      - MAX_CPU_WORKERS=
    deploy:
      resources:
        reservations:
          devices:
            - driver: nvidia
              count: all
              capabilities: [gpu]
    restart: unless-stopped
```

## Environment variables

| Variable | Default | Description |
|----------|---------|-------------|
| `APP_BIND` | `0.0.0.0:8080` | Server listen address |
| `DATABASE_PATH` | `/data/app.db` | SQLite database path |
| `TZ` | `UTC` | Timezone for schedules |
| `RESERVED_CPU_CORES` | `1` | Cores reserved for UI/container |
| `MAX_CPU_WORKERS` | (auto) | Max CPU worker threads |

## Development

```bash
# Run tests
docker compose -f docker-compose.dev.yml run --rm dev cargo test

# Or inside the dev container
docker run --rm -v $PWD:/build -w /build meshcore-keygen-dev-builder cargo test --features server

# Format
cargo fmt

# Lint
cargo clippy --no-deps
```

## Security

- Private keys and seeds are hidden by default in the UI and logs
- Use `APP_PASSWORD` env var for basic auth (future)
- Do not expose the web UI to the public internet without a reverse proxy
- Back up `/data` to preserve jobs, results, and settings

## License

MIT OR Apache-2.0

## Sources

- [MeshCore](https://github.com/ripplebiz/MeshCore)
- [MeshCore mc-keygen web tool](https://gessaman.com/mc-keygen/)
- [Ed25519 / RFC 8032](https://datatracker.ietf.org/doc/html/rfc8032)
- [curve25519-dalek](https://github.com/dalek-cryptography/curve25519-dalek)
- [ratatui](https://github.com/ratatui/ratatui)
