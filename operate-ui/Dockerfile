# Multi-stage build for operate-ui
# Stage 1: cargo-chef planner for dependency caching
# Pin rust:alpine to specific digest for supply chain security
FROM rust:alpine@sha256:a3e3d30122c08c0ed85dcd8867d956f066be23c32ed67a0453bc04ce478ad69b AS chef
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
COPY registry/ ./registry/
COPY operate-ui/ ./operate-ui/
COPY tooling/ ./tooling/
COPY wards/ ./wards/
COPY capsules/ ./capsules/
COPY demonctl/ ./demonctl/
COPY bootstrapper/ ./bootstrapper/
COPY controller/ ./controller/
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
COPY registry/ ./registry/
COPY operate-ui/ ./operate-ui/
COPY tooling/ ./tooling/
COPY wards/ ./wards/
COPY capsules/ ./capsules/
COPY controller/ ./controller/
COPY demonctl/ ./demonctl/
COPY bootstrapper/ ./bootstrapper/

ENV RUSTFLAGS="-C target-feature=+crt-static"
RUN case "$TARGETPLATFORM" in \
      "linux/amd64") cargo build --release --bin operate-ui --target x86_64-unknown-linux-musl ;; \
      "linux/arm64") cargo build --release --bin operate-ui --target aarch64-unknown-linux-musl ;; \
    esac

# Runtime stage - use static distroless for musl-built binaries
# Pin distroless to specific digest for supply chain security
FROM gcr.io/distroless/static-debian12@sha256:87bce11be0af225e4ca761c40babb06d6d559f5767fbf7dc3c47f0f1a466b92c

# Copy template files
COPY --from=builder /app/operate-ui/templates /app/templates

# Copy the binary (wildcard matches either x86_64 or aarch64)
COPY --from=builder /app/target/*/release/operate-ui /usr/local/bin/operate-ui

# Set working directory
WORKDIR /app

# Expose port
EXPOSE 3000

# Run as non-root
USER nonroot:nonroot

# Start the application
CMD ["/usr/local/bin/operate-ui"]
