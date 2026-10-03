# Build UI
FROM node:20-alpine AS ui
WORKDIR /ui
COPY ui/package.json ui/package-lock.json* ./
RUN npm install
COPY ui/ ./
RUN npm run build

# Build server
FROM rust:1.89-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo build --release -p thine-server

# Runtime
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
  && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/thine-server /usr/local/bin/thine-server
COPY --from=ui /ui/dist /app/ui/dist
ENV THINE_HOST=0.0.0.0 \
    THINE_PORT=4318 \
    THINE_UI_DIR=/app/ui/dist \
    THINE_SEED_DEMO=true
EXPOSE 4318
CMD ["thine-server"]
