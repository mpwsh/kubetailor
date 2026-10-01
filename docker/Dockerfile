# syntax=docker/dockerfile:1

# Keep this in step with rust-toolchain.toml. The builder and the runtime MUST be the same
# Debian release: the binaries link against the builder's libssl (openssl-tls in kube,
# native-tls in redis/portier), and bullseye ships OpenSSL 1.1 while bookworm ships OpenSSL 3.
ARG RUST_VERSION=1.82.0
ARG DEBIAN_RELEASE=bookworm

# rust:<version>-<release> with cargo-chef preinstalled, so it is not compiled on every cold build.
FROM lukemathwalker/cargo-chef:latest-rust-${RUST_VERSION}-${DEBIAN_RELEASE} AS chef
WORKDIR /app
# Pin the toolchain once, so every stage below agrees with the image tag.
COPY rust-toolchain.toml rust-toolchain.toml
RUN apt-get update -y \
    && apt-get install -y --no-install-recommends lld clang libssl-dev pkg-config \
    && rm -rf /var/lib/apt/lists/*

FROM chef AS planner
COPY Cargo.toml Cargo.lock ./
COPY crates crates
# Compute a lock-like file for our project
RUN cargo chef prepare --recipe-path recipe.json

# One builder for all three binaries: the workspace crates compile once instead of once per
# target, and CI exports a single layer cache that every final image shares.
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
# Build our project dependencies, not our application!
RUN cargo chef cook --release --recipe-path recipe.json
COPY Cargo.toml Cargo.lock ./
COPY crates crates
RUN cargo build --release --locked --bin operator --bin server --bin console

# Runtime: same Debian release as the builder (see ARG DEBIAN_RELEASE above).
# libssl3 is the runtime library the binaries dlopen; libssl-dev is only needed to compile.
FROM debian:${DEBIAN_RELEASE}-slim AS base
WORKDIR /app
RUN apt-get update -y \
    && apt-get install -y --no-install-recommends libssl3 ca-certificates \
    && apt-get autoremove -y \
    && apt-get clean -y \
    && rm -rf /var/lib/apt/lists/*

FROM base AS console
COPY web /app/web
COPY --from=builder /app/target/release/console console
ENTRYPOINT ["./console"]

FROM base AS server
COPY --from=builder /app/target/release/server server
ENTRYPOINT ["./server"]

FROM base AS operator
COPY --from=builder /app/target/release/operator operator
ENTRYPOINT ["./operator"]
