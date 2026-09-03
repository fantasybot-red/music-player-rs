# ----------------------------------------------------
# Stage 1: Build stage
# ----------------------------------------------------
FROM rust:1.98 AS builder

# Install system dependencies, including NASM and Git
RUN apt-get update && apt-get install -y \
    cmake \
    nasm \
    git \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

COPY . .
RUN cargo build --release

# ----------------------------------------------------
# Stage 2: Tiny Runtime stage
# ----------------------------------------------------
FROM debian:bookworm-slim

# Install runtime SSL certs if your app makes HTTPS requests
RUN apt-get update && apt-get install -y \
    ca-certificates \
    ffmpeg \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

ENV IS_DOCKER=true

COPY --from=builder /build/target/release/music_players /app/music_players

CMD ["./music_players"]
