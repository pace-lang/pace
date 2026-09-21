# Stage 1: Build the Pace compiler
FROM rust:1.98 AS builder

WORKDIR /usr/src/pace

# Copy the source code
COPY . .

# Build the compiler in release mode
WORKDIR /usr/src/pace/compiler
RUN cargo build --release

# Stage 2: Minimal runtime image
FROM debian:bookworm-slim

# Link the image to the repository
LABEL org.opencontainers.image.source=https://github.com/pace-lang/pace
LABEL org.opencontainers.image.description="Pace Programming Language Compiler"
LABEL org.opencontainers.image.licenses="MIT"

# Install gcc and libc dev headers needed for linking Pace programs
RUN apt-get update && \
    apt-get install -y gcc libc6-dev && \
    rm -rf /var/lib/apt/lists/*

# Copy the compiled compiler binary from the builder stage
COPY --from=builder /usr/src/pace/compiler/target/release/pace /usr/local/bin/pace

# Set the working directory to where users will mount their code
WORKDIR /workspace

# Set pace as the entrypoint so the container acts like the CLI tool
ENTRYPOINT ["pace"]
