# syntax=docker/dockerfile:1

# Build static console assets.
FROM node:22.12.0-bookworm-slim AS web-builder
WORKDIR /build
ENV PNPM_HOME=/pnpm
ENV PATH="$PNPM_HOME:$PATH"
RUN corepack enable && corepack prepare pnpm@11.15.1 --activate
COPY web/package.json web/pnpm-lock.yaml web/pnpm-workspace.yaml ./
COPY web/packages ./packages
COPY web/apps ./apps
RUN pnpm install --frozen-lockfile
RUN pnpm -r build

# Build the Rust platform binary.
FROM rust:1.96.1-slim-bookworm AS rust-builder
WORKDIR /build
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config libssl-dev ca-certificates
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY apps ./apps
COPY tools ./tools
COPY config.example.toml ./
RUN cargo build --release -p security-platform

# Runtime image with non-root user and read-only rootfs support.
FROM gcr.io/distroless/cc-debian12:nonroot
WORKDIR /app
COPY --from=rust-builder /build/target/release/security-platform /app/security-platform
COPY --from=web-builder /build/apps/console/dist /var/www/static
ENV CLSC_STATIC_DIR=/var/www/static
EXPOSE 8080
USER nonroot:nonroot
HEALTHCHECK --interval=30s --timeout=5s --start-period=5s --retries=3 \
  CMD ["/app/security-platform", "health"]
ENTRYPOINT ["/app/security-platform"]
