FROM rust:bullseye as builder
COPY ./ ./
RUN cargo build --bin qos-core --features vm --no-default-features

# We don't need the Rust toolchain to run the binary!
FROM rust:bullseye AS runtime
WORKDIR app
RUN apt-get update \
  && apt-get install -y libssl-dev
COPY --from=builder /target/debug/qos-core /usr/local/bin
RUN ls -a /lib/
RUN ls -a /lib/x86_64-linux-gnu/
# ENTRYPOINT ["/usr/local/bin/qos-core", "--port", "6969", "--cid", "16"]
ENTRYPOINT ["ls", "-a", "/lib/x86_64-linux-gnu/"]


# Leveraging the pre-built Docker images with 
# cargo-chef and the Rust toolchain
# FROM lukemathwalker/cargo-chef:latest AS chef
# WORKDIR app

# FROM chef AS planner
# COPY . .
# RUN cargo chef prepare --recipe-path recipe.json

# FROM chef AS builder
# COPY --from=planner /app/recipe.json recipe.json
# # Build dependencies - this is the caching Docker layer!
# RUN cargo chef cook --release --recipe-path recipe.json
# # Build application
# COPY . .
# RUN cargo build --bin qos-core --release --features vm --no-default-features

# # We do not need the Rust toolchain to run the binary!
# FROM debian:bullseye-slim AS runtime
# WORKDIR app
# # COPY --from=builder /app/target/release/app /usr/local/bin
# RUN apt-get update \
#   && apt-get install -y libssl-dev \
#   && apt-get install libc6-dev -y
# COPY --from=builder /app/target/release/qos-core /usr/local/bin
# ENTRYPOINT ["/usr/local/bin/qos-core", "--port", "6969", "--cid", "16"]

