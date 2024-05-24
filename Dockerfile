FROM rust:1.78 AS chef
# We only pay the installation cost once,
# it will be cached from the second build onwards
RUN cargo install cargo-chef
WORKDIR app

FROM chef AS planner
COPY . .
RUN cargo chef prepare  --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --release --recipe-path recipe.json
# Build application
COPY . .
RUN cargo build --release --bin opentsdb-auth-proxy

# We do not need the Rust toolchain to run the binary!
FROM ubuntu:22.04
WORKDIR app
COPY --from=builder /app/target/release/opentsdb-auth-proxy /usr/local/bin
ENTRYPOINT ["/usr/local/bin/opentsdb-auth-proxy"]