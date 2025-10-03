# Multi-stage build for engine
# Stage 1: cargo-chef planner for dependency caching
FROM --platform=$BUILDPLATFORM rust:alpine AS chef
RUN apk add --no-cache musl-dev openssl-dev openssl-libs-static gcc
RUN cargo install cargo-chef
WORKDIR /app

# Stage 2: Analyze dependencies
FROM chef AS planner
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates/ ./crates/
COPY contracts/ ./contracts/
COPY runtime/ ./runtime/
COPY engine/ ./engine/
COPY operate-ui/ ./operate-ui/
COPY wards/ ./wards/
COPY capsules/ ./capsules/
COPY demonctl/ ./demonctl/
COPY bootstrapper/ ./bootstrapper/
RUN cargo chef prepare --recipe-path recipe.json

# Stage 3: Build dependencies (cached layer)
FROM chef AS builder
ARG TARGETPLATFORM
# Install musl target for the platform we're building
RUN case "$TARGETPLATFORM" in \
      "linux/amd64") rustup target add x86_64-unknown-linux-musl ;; \
      "linux/arm64") rustup target add aarch64-unknown-linux-musl ;; \
      *) echo "Unsupported platform: $TARGETPLATFORM" && exit 1 ;; \
    esac
COPY --from=planner /app/recipe.json recipe.json
RUN case "$TARGETPLATFORM" in \
      "linux/amd64") cargo chef cook --release --target x86_64-unknown-linux-musl --recipe-path recipe.json ;; \
      "linux/arm64") cargo chef cook --release --target aarch64-unknown-linux-musl --recipe-path recipe.json ;; \
    esac

# Stage 4: Build application
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates/ ./crates/
COPY contracts/ ./contracts/
COPY runtime/ ./runtime/
COPY engine/ ./engine/
COPY operate-ui/ ./operate-ui/
COPY wards/ ./wards/
COPY capsules/ ./capsules/
COPY demonctl/ ./demonctl/
COPY bootstrapper/ ./bootstrapper/

ENV RUSTFLAGS="-C target-feature=+crt-static"
RUN case "$TARGETPLATFORM" in \
      "linux/amd64") cargo build --release --bin engine --target x86_64-unknown-linux-musl ;; \
      "linux/arm64") cargo build --release --bin engine --target aarch64-unknown-linux-musl ;; \
    esac

# Runtime stage - use static distroless for musl-built binaries
FROM gcr.io/distroless/static-debian12

# Copy the binary (wildcard matches either x86_64 or aarch64)
COPY --from=builder /app/target/*/release/engine /usr/local/bin/engine

# Expose port
EXPOSE 8081

# Run as non-root
USER nonroot:nonroot

# Start the application
CMD ["/usr/local/bin/engine"]