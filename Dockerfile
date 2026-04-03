# ============================================================
# Dockerfile — AgentX-Sprint2
# Build: docker build --platform linux/amd64 -t ghcr.io/tenalirama2005/agentx-sprint2:latest .
# ============================================================
FROM rust:slim AS builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src

# Build release binary
RUN apt-get update && apt-get install -y pkg-config libssl-dev && \
    cargo build --release --locked && \
    strip target/release/agentx-sprint2

# ── Runtime stage ────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/agentx-sprint2 .

ENV PORT=8090
ENV RUST_LOG=agentx_sprint2=info,tower_http=warn

EXPOSE 8090

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s \
    CMD curl -f http://localhost:8090/health || exit 1

ENTRYPOINT ["./agentx-sprint2"]
