FROM rust:1-bookworm AS builder

WORKDIR /usr/src/kasane

COPY . .

RUN cargo build --release --features production

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    tzdata \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /usr/src/kasane/target/release/kasane /app/kasane

ENV DATABASE_DIR=/data
ENV PORT=5172
ENV RUST_LOG=info

VOLUME ["/data"]

EXPOSE 5172

# 実行
CMD ["./kasane"]
