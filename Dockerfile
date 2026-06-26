# ── Stage 1: dependency planner ───────────────────────────────────────────────
FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ── Stage 2: build ────────────────────────────────────────────────────────────
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
# Cache dependency compilation as a separate Docker layer
RUN cargo chef cook --release --recipe-path recipe.json
# Build the actual binary
COPY . .
RUN cargo build --release --bin server

# ── Stage 3: minimal runtime image ────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/server /app/server

EXPOSE 8080

ENV SERVER_HOST=0.0.0.0
ENV SERVER_PORT=8080

HEALTHCHECK --interval=30s --timeout=10s --start-period=40s --retries=3 \
    CMD curl -f http://localhost:8080/health/ready || exit 1

CMD ["/app/server"]
