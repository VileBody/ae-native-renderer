FROM rust:1-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    clang \
    cmake \
    pkg-config \
    ca-certificates \
    git \
    libgstreamer1.0-dev \
    libgstreamer-plugins-base1.0-dev \
    libglib2.0-dev \
    libfreetype6-dev \
    libharfbuzz-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY . .
RUN cargo build --release -p render-cli -p render-manager

FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    ffmpeg \
    gstreamer1.0-tools \
    gstreamer1.0-plugins-base \
    gstreamer1.0-plugins-good \
    gstreamer1.0-plugins-bad \
    gstreamer1.0-plugins-ugly \
    gstreamer1.0-libav \
    libgstreamer1.0-0 \
    libgstreamer-plugins-base1.0-0 \
    libglib2.0-0 \
    libfreetype6 \
    libharfbuzz0b \
    fontconfig \
    curl \
    podman \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /work
COPY --from=builder /app/target/release/render-cli /usr/local/bin/render-cli
COPY --from=builder /app/target/release/render-manager /usr/local/bin/render-manager

ENTRYPOINT ["render-cli"]
