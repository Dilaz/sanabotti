FROM rust:latest AS builder

WORKDIR /app

# Install necessary build dependencies
RUN apt-get update && \
    apt-get install -y \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

# Copy Cargo.toml and Cargo.lock
COPY Cargo.toml Cargo.lock ./

# Create a dummy main.rs to build dependencies
RUN mkdir -p src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Copy actual source code
COPY src/ src/
COPY data/ data/

# Build the actual application
RUN cargo build --release

# Create the runtime container
FROM gcr.io/distroless/cc-debian12

WORKDIR /app

# Copy the built binary from the builder stage
COPY --from=builder /app/target/release/sanabotti /app/sanabotti

# Copy data directory
COPY data/ /app/data/

# Create a volume for configuration
VOLUME /app/config

# Set the entrypoint
ENTRYPOINT ["/app/sanabotti"] 