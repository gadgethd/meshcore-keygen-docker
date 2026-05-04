# --- Build stage ---
FROM nvidia/cuda:12.6.3-devel-ubuntu24.04 AS builder

ENV DEBIAN_FRONTEND=noninteractive

# Switch to HTTPS Ubuntu mirrors (HTTP is throttled/slow)
RUN sed -i 's|http://archive.ubuntu.com|https://archive.ubuntu.com|g' /etc/apt/sources.list.d/ubuntu.sources && \
    sed -i 's|http://security.ubuntu.com|https://security.ubuntu.com|g' /etc/apt/sources.list.d/ubuntu.sources && \
    sed -i 's|http://ports.ubuntu.com|https://ports.ubuntu.com|g' /etc/apt/sources.list.d/ubuntu.sources

# Build dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl ca-certificates build-essential pkg-config \
    libssl-dev libsqlite3-dev \
    && rm -rf /var/lib/apt/lists/*

# Install Rust
ENV RUSTUP_HOME=/usr/local/rustup CARGO_HOME=/usr/local/cargo
ENV PATH=/usr/local/cargo/bin:$PATH
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --default-toolchain stable --profile minimal \
    && rustup component add clippy rustfmt

WORKDIR /build

# Cache dependencies in a separate layer
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src && echo 'fn main() {}' > src/main.rs \
    && cargo fetch || true \
    && rm -rf src

# Copy source
COPY . .

# Build with CUDA + server support
RUN cargo build --release --features cuda,server 2>&1 || \
    cargo build --release --features server 2>&1 || \
    cargo build --release 2>&1

# --- Runtime stage ---
FROM nvidia/cuda:12.6.3-runtime-ubuntu24.04 AS runtime

ENV DEBIAN_FRONTEND=noninteractive

# Switch to HTTPS Ubuntu mirrors
RUN sed -i 's|http://archive.ubuntu.com|https://archive.ubuntu.com|g' /etc/apt/sources.list.d/ubuntu.sources && \
    sed -i 's|http://security.ubuntu.com|https://security.ubuntu.com|g' /etc/apt/sources.list.d/ubuntu.sources && \
    sed -i 's|http://ports.ubuntu.com|https://ports.ubuntu.com|g' /etc/apt/sources.list.d/ubuntu.sources

# Runtime deps: libssl, libsqlite3, CUDA runtime for nvrtc
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl libssl3 libsqlite3-0 \
    cuda-cudart-12-6 cuda-nvrtc-12-6 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /build/target/release/mc-keygen /app/mc-keygen

# Default data directory
RUN mkdir -p /data
VOLUME /data

ENV APP_BIND=0.0.0.0:8080
ENV RESERVED_CPU_CORES=1

EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/api/status || exit 1

ENTRYPOINT ["/app/mc-keygen"]
CMD ["--serve"]
