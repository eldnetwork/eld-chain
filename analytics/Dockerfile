# GHCR and local image for eld-analytics. Build context is the repository root.
FROM rust:1.88.0-bookworm AS build

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    clang \
    libclang-dev \
    pkg-config \
    libssl-dev \
    libudev-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY common ./common
COPY analytics ./analytics
# Cargo loads every workspace member. Drop members that were not copied.
RUN sed -i 's/, "cli"//g; s/, "node_app"//g; s/, "client"//g; s/, "faucet"//g' Cargo.toml \
    && cargo metadata --format-version 1 >/dev/null \
    && cargo build --release --locked -p eld-analytics

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    libstdc++6 \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd -r -g 1000 elduser \
    && useradd -r -u 1000 -g elduser elduser \
    && mkdir -p /data \
    && chown -R elduser:elduser /data

COPY --from=build /app/target/release/eld-analytics /usr/local/bin/eld-analytics
RUN chmod 755 /usr/local/bin/eld-analytics \
    && ldd /usr/local/bin/eld-analytics \
    && ! ldd /usr/local/bin/eld-analytics | grep -q 'not found'

WORKDIR /data
USER elduser
EXPOSE 9002
ENTRYPOINT ["eld-analytics"]
