# Stage 1: Build the Pace compiler
FROM rust:alpine AS builder

# Install build dependencies for Alpine
RUN apk add --no-cache musl-dev gcc

WORKDIR /usr/src/pace

# Copy the source code
COPY . .

# Build the compiler in release mode
WORKDIR /usr/src/pace/compiler
RUN cargo build --release

# Stage 2: Minimal runtime image
FROM alpine:latest

# Link the image to the repository
LABEL org.opencontainers.image.source=https://github.com/pace-lang/pace
LABEL org.opencontainers.image.description="Pace Programming Language Compiler"
LABEL org.opencontainers.image.licenses="MIT"

# Install gcc and musl dev headers needed for linking Pace programs
RUN apk add --no-cache gcc musl-dev

# Copy the compiled compiler binary from the builder stage
COPY --from=builder /usr/src/pace/compiler/target/release/pace /usr/local/bin/pace

# Set the working directory to where users will mount their code
WORKDIR /workspace

# Set pace as the entrypoint so the container acts like the CLI tool
ENTRYPOINT ["pace"]
