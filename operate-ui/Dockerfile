# Multi-stage build for operate-ui
FROM rust:1.77-alpine AS builder

# Install build dependencies
RUN apk add --no-cache musl-dev openssl-dev openssl-libs-static

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
COPY bootstrapper/ ./bootstrapper/

# Set build target for static linking
ENV RUSTFLAGS="-C target-feature=-crt-static"

# Build the operate-ui binary
RUN cargo build --release --bin operate-ui

# Runtime stage
FROM gcr.io/distroless/cc-debian12

# Copy template files
COPY --from=builder /app/operate-ui/templates /app/templates

# Copy the binary
COPY --from=builder /app/target/release/operate-ui /usr/local/bin/operate-ui

# Set working directory
WORKDIR /app

# Expose port
EXPOSE 3000

# Run as non-root
USER nonroot:nonroot

# Start the application
CMD ["operate-ui"]