# Multi-stage build for operate-ui
FROM rust:1.77-alpine AS builder

# Install build dependencies
RUN apk add --no-cache musl-dev openssl-dev openssl-libs-static

# Add musl target for static linking
RUN rustup target add x86_64-unknown-linux-musl

WORKDIR /app

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY rust-toolchain.toml ./

# Update toolchain to match rust-toolchain.toml and add musl target
RUN rustup set profile minimal \
 && rustup update --no-self-update stable \
 && rustup target add --toolchain stable x86_64-unknown-linux-musl

# Copy all workspace members and contracts (needed for dependencies)
COPY crates/ ./crates/
COPY contracts/ ./contracts/
COPY runtime/ ./runtime/
COPY engine/ ./engine/
COPY operate-ui/ ./operate-ui/
COPY wards/ ./wards/
COPY capsules/ ./capsules/
COPY demonctl/ ./demonctl/
COPY bootstrapper/ ./bootstrapper/

# Set build target for fully static linking
ENV RUSTFLAGS="-C target-feature=+crt-static"

# Build the operate-ui binary
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