# syntax=docker/dockerfile:1

# ─── Rust build ───────────────────────────────────────────────────
FROM rust:1.94-bookworm AS rust-builder
WORKDIR /app
COPY . .
RUN cargo build --release --bin riichi-api --bin riichi-worker

# ─── Web build ────────────────────────────────────────────────────
FROM node:22-bookworm AS web-builder
WORKDIR /app
RUN corepack enable && corepack prepare pnpm@10.33.0 --activate

# Install deps (cached layer — only rebuilds when lockfile changes)
COPY package.json pnpm-workspace.yaml pnpm-lock.yaml ./
COPY apps/web/package.json apps/web/
RUN pnpm install --frozen-lockfile

# Build the SPA
COPY apps/web/ apps/web/
ARG VITE_ELECTRIC_SYNC_ENABLED=true
ENV VITE_ELECTRIC_SYNC_ENABLED=$VITE_ELECTRIC_SYNC_ENABLED
RUN pnpm build

# ─── API runtime ────────────────────────────────────────────────
FROM debian:bookworm-slim AS api
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=rust-builder /app/target/release/riichi-api /app/riichi-api
CMD ["./riichi-api"]

# ─── Worker runtime ─────────────────────────────────────────────
FROM debian:bookworm-slim AS worker
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=rust-builder /app/target/release/riichi-worker /app/riichi-worker
CMD ["./riichi-worker"]

# ─── Web (nginx serves SPA, proxies to API) ──────────────────────
FROM nginx:stable AS web
COPY --from=web-builder /app/apps/web/dist /usr/share/nginx/html
COPY docker/nginx.conf /etc/nginx/conf.d/default.conf
