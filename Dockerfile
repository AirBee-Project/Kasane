FROM rust:1-bookworm AS chef

RUN apt-get update && apt-get install -y cmake clang && rm -rf /var/lib/apt/lists/*

RUN cargo install cargo-chef
WORKDIR /app


FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json

RUN cargo chef cook --release --features production --recipe-path recipe.json

COPY . .
RUN cargo build --release --features production

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    tzdata \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/kasane /app/kasane

ENV DATABASE_DIR=/data
ENV PORT=5172
ENV RUST_LOG=info

VOLUME ["/data"]

EXPOSE 5172

CMD ["./kasane"]
