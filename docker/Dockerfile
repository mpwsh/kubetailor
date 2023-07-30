FROM rust:1.69.0-bookworm as chef
RUN cargo install cargo-chef --locked

WORKDIR /app
RUN apt update && apt install lld clang libssl-dev pkg-config -y

FROM chef as planner
COPY Cargo.* ./
COPY crates crates
# Compute a lock-like file for our project
RUN cargo chef prepare  --recipe-path recipe.json

FROM chef as builder
COPY --from=planner /app/recipe.json recipe.json
# Build our project dependencies, not our application!
RUN cargo chef cook --release --recipe-path recipe.json
COPY crates crates

FROM builder AS builder-console
RUN cargo build --release --bin console

FROM builder AS builder-server
RUN cargo build --release --bin server

FROM builder AS builder-operator
RUN cargo build --release --bin operator

FROM debian:bookworm-slim AS base
WORKDIR /app
RUN apt-get update -y \
    && apt-get install -y --no-install-recommends libssl-dev ca-certificates \
    # Clean up
    && apt-get autoremove -y \
    && apt-get clean -y \
    && rm -rf /var/lib/apt/lists/*

FROM base AS console
COPY crates/console/static /app/static
COPY crates/console/templates /app/templates
COPY --from=builder-console /app/target/release/console console
ENTRYPOINT ["./console"]

FROM base AS server
COPY --from=builder-server /app/target/release/server server
ENTRYPOINT ["./server"]

FROM base AS operator
COPY --from=builder-operator /app/target/release/operator operator
ENTRYPOINT ["./operator"]
