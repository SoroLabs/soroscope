# ------------------------------------------------------------------------------
# Stage 1: Cargo Chef Base
# ------------------------------------------------------------------------------
FROM lukemathwalker/cargo-chef:latest-rust-1-alpine AS chef
WORKDIR /app

# Install build-time dependencies (protoc for tonic-build, pkgconfig, openssl, C build tools)
RUN apk add --no-cache \
    musl-dev \
    pkgconfig \
    openssl-dev \
    protoc \
    protobuf-dev \
    build-base

# ------------------------------------------------------------------------------
# Stage 2: Planner (Generate recipe.json for dependency caching)
# ------------------------------------------------------------------------------
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ------------------------------------------------------------------------------
# Stage 3: Builder (Cook dependencies and compile release binary)
# ------------------------------------------------------------------------------
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json

# Build dependency cache layer
RUN cargo chef cook --release --recipe-path recipe.json --bin soroscope-core

# Copy source tree and compile core server binary
COPY . .
RUN cargo build --release --bin soroscope-core && \
    strip /app/target/release/soroscope-core

# ------------------------------------------------------------------------------
# Stage 4: Minimal Runtime Image (< 50 MB)
# ------------------------------------------------------------------------------
FROM alpine:3.20 AS runtime

# Install minimal runtime dependencies (certificates for HTTPS/TLS, timezone data, libgcc)
RUN apk add --no-cache \
    ca-certificates \
    tzdata \
    libgcc

# Create unprivileged application user & group
RUN addgroup -S soroscope && adduser -S -G soroscope soroscope

WORKDIR /app

# Copy stripped binary from builder stage
COPY --from=builder --chown=soroscope:soroscope /app/target/release/soroscope-core /usr/local/bin/soroscope-core

# Set non-root execution
USER soroscope

# Expose default HTTP/gRPC service port
EXPOSE 8080

# Configure environment defaults
ENV RUST_LOG=info \
    PORT=8080

ENTRYPOINT ["soroscope-core"]