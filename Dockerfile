# syntax=docker/dockerfile:1

# ─────────────────────────────────────────────
# Build stage
# F10: Using rust 1.88 (aligned with Cargo.lock resolution).
#      Cargo.toml rust-version=1.75 sets the MSRV; Docker uses latest stable.
# ─────────────────────────────────────────────
FROM rust:1.88-slim-bookworm AS builder

WORKDIR /src

# Install musl target for static linking
RUN rustup target add x86_64-unknown-linux-musl \
    && apt-get update && apt-get install -y --no-install-recommends \
    musl-tools \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /src

# Install build dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy dependency definitions first (for layer caching)
COPY Cargo.toml ./
# Touch dummy source so cargo can resolve deps
RUN mkdir -p src && echo "fn main() {}" > src/main.rs
# Generate Cargo.lock if it doesn't exist
RUN cargo generate-lockfile || true
COPY Cargo.lock* ./

# Build dependencies only (gnu target for speed)
RUN cargo build --release && rm -rf src

# Copy actual source and build musl static binary
COPY . .
RUN cargo build --release --target x86_64-unknown-linux-musl

# Strip the binary
RUN strip target/x86_64-unknown-linux-musl/release/jwt-exchange

# ─────────────────────────────────────────────
# Runtime stage — distroless/static (~2MB)
#
# No shell, no package manager, no extra tools.
# Just the binary and CA certificates for TLS.
# ─────────────────────────────────────────────
FROM gcr.io/distroless/static-debian12:nonroot

# F9: Run as non-root user (distroless nonroot: UID 65532)
USER nonroot

# Copy the compiled musl static binary
COPY --from=builder /src/target/x86_64-unknown-linux-musl/release/jwt-exchange /usr/local/bin/jwt-exchange

# F24: Persist SQLite database to a mountable volume path.
#      The container must mount a volume at /data to survive restarts.
ENV DB_PATH=/data/jwt-exchange.db
ENV RUST_LOG=info

# Expose the service port
EXPOSE 8080

# The distroless image doesn't have curl for HEALTHCHECK
# Health is checked via /health endpoint by the orchestrator

ENTRYPOINT ["/usr/local/bin/jwt-exchange"]
