# ストレージバックエンドはビルドごとに 1 つしか選べない（`src/backend.rs`）ので、
# イメージもバックエンドごとに 1 つ焼く。差分はこの ARG だけ。
#
#   docker build -t kasane:lmdb .
#   docker build -t kasane:tikv --build-arg BACKEND=backend-tikv .
ARG BACKEND=backend-lmdb

FROM rust:1-bookworm AS chef

RUN apt-get update && apt-get install -y cmake clang && rm -rf /var/lib/apt/lists/*

RUN cargo install cargo-chef
WORKDIR /app


FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
ARG BACKEND
# 依存のビルドと本体のビルドで feature が食い違うと、せっかくの依存キャッシュが
# 使われずに全部ビルドし直しになる。同じ値を両方へ渡す。
ENV FEATURES="production,${BACKEND}"

COPY --from=planner /app/recipe.json recipe.json

RUN cargo chef cook --release --no-default-features --features "${FEATURES}" --recipe-path recipe.json

COPY . .
RUN cargo build --release --no-default-features --features "${FEATURES}"

FROM debian:bookworm-slim
ARG BACKEND

RUN apt-get update && apt-get install -y \
    ca-certificates \
    tzdata \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/kasane /app/kasane

# どのバックエンドで焼いたイメージかを外から確認できるようにしておく。
LABEL org.opencontainers.image.title="kasane"
LABEL io.kasane.backend="${BACKEND}"

# LMDB のときはデータディレクトリ、TiKV のときは未使用（接続先は
# KASANE_TIKV_PD_ENDPOINTS で指定する）。
ENV DATABASE_DIR=/data
ENV PORT=5172
ENV RUST_LOG=info

VOLUME ["/data"]

EXPOSE 5172

CMD ["./kasane"]
