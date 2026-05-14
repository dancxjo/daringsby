# syntax=docker/dockerfile:1.7

ARG CUDA_VERSION=12.4.1

FROM nvidia/cuda:${CUDA_VERSION}-devel-bookworm AS build

ARG PETE_BIN

ENV CARGO_HOME=/usr/local/cargo \
    RUSTUP_HOME=/usr/local/rustup \
    PATH=/usr/local/cargo/bin:$PATH

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        build-essential \
        ca-certificates \
        clang \
        cmake \
        curl \
        libssl-dev \
        pkg-config \
    && rm -rf /var/lib/apt/lists/*

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --profile minimal --default-toolchain stable

WORKDIR /app

COPY . .

RUN test -n "$PETE_BIN"

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cargo build -p pete --release --features scene-vec --bin "$PETE_BIN" \
    && cp "target/release/$PETE_BIN" /usr/local/bin/pete-component

FROM nvidia/cuda:${CUDA_VERSION}-devel-bookworm AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        libgomp1 \
        libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=build /usr/local/bin/pete-component /usr/local/bin/pete-component
COPY frontend/dist /app/frontend/dist
COPY frontend/psychic /app/frontend/psychic

ENTRYPOINT ["/usr/local/bin/pete-component"]
