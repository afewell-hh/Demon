# Multi-stage build for runtime
FROM rust:1.77-alpine AS builder

# Install build dependencies
RUN apk add --no-cache musl-dev openssl-dev openssl-libs-static

# Add musl target for static linking
RUN rustup target add x86_64-unknown-linux-musl

WORKDIR /app

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY rust-toolchain.toml ./

# Copy all workspace members and contracts (needed for dependencies)
COPY crates/ ./crates/
COPY contracts/ ./contracts/
COPY runtime/ ./runtime/
COPY engine/ ./engine/
COPY operate-ui/ ./operate-ui/
COPY wards/ ./wards/
COPY capsules/ ./capsules/
COPY demonctl/ ./demonctl/

# Set build target for fully static linking
ENV RUSTFLAGS="-C target-feature=+crt-static"

# Build the runtime binary
RUN cargo build --release --bin runtime --target x86_64-unknown-linux-musl

# Runtime stage - use static distroless for musl-built binaries
FROM gcr.io/distroless/static-debian12

# Copy the binary
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/runtime /usr/local/bin/runtime

# Expose port
EXPOSE 8080

# Run as non-root
USER nonroot:nonroot

# Start the application
CMD ["/usr/local/bin/runtime"]