# Multi-stage build for operate-ui
# Stage 1: cargo-chef planner for dependency caching
FROM rust:alpine AS chef
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
RUN rustup target add x86_64-unknown-linux-musl
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --target x86_64-unknown-linux-musl --recipe-path recipe.json

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
RUN cargo build --release --bin operate-ui --target x86_64-unknown-linux-musl

# Runtime stage - use static distroless for musl-built binaries
FROM gcr.io/distroless/static-debian12

# Copy template files
COPY --from=builder /app/operate-ui/templates /app/templates

# Copy the binary
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/operate-ui /usr/local/bin/operate-ui

# Set working directory
WORKDIR /app

# Expose port
EXPOSE 3000

# Run as non-root
USER nonroot:nonroot

# Start the application
CMD ["/usr/local/bin/operate-ui"]