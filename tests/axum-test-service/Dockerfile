ARG RUST_VERSION

FROM rust:${RUST_VERSION}-bullseye as builder

WORKDIR /app

RUN apt-get update && apt-get install -y \
    build-essential \
    pkg-config \
    libssl-dev

COPY . .

RUN cargo build --release -p treblle-axum-test-service

FROM debian:bullseye-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl1.1 \
    && rm -rf /var/lib/apt/lists/*

# Copy the binary from the builder stage
COPY --from=builder /app/target/release/treblle-axum-test-service /usr/local/bin/

CMD ["treblle-axum-test-service"]