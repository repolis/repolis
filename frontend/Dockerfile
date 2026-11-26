# syntax=docker/dockerfile:1
FROM rust:1.97.1-slim AS engine-builder
ARG WASM_PACK=0.15.1
RUN apt-get update && apt-get install -y --no-install-recommends curl ca-certificates && rm -rf /var/lib/apt/lists/*
RUN curl -sSfL "https://github.com/wasm-bindgen/wasm-pack/releases/download/v${WASM_PACK}/wasm-pack-v${WASM_PACK}-$(uname -m)-unknown-linux-musl.tar.gz" \
    | tar -xz --strip-components=1 -C /usr/local/bin "wasm-pack-v${WASM_PACK}-$(uname -m)-unknown-linux-musl/wasm-pack"
WORKDIR /src/engine
COPY engine/rust-toolchain.toml ./
RUN rustup toolchain install
COPY engine/ ./
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/engine/target \
    ./build-wasm.sh

FROM node:22-slim AS frontend-builder
WORKDIR /src/frontend
COPY frontend/package.json frontend/package-lock.json ./
RUN --mount=type=cache,target=/root/.npm npm ci
COPY frontend/ ./
COPY --from=engine-builder /src/frontend/src/wasm ./src/wasm
RUN npm run build

FROM nginx:1.30-alpine
COPY frontend/nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=frontend-builder /src/frontend/dist /usr/share/nginx/html
EXPOSE 80
