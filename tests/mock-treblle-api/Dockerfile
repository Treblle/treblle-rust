ARG RUST_VERSION=1.82.0

FROM rust:${RUST_VERSION}-bullseye as builder

WORKDIR /app

RUN apt-get update && apt-get install -y \
    build-essential \
    pkg-config \
    libssl-dev

COPY . .

RUN cargo build --release -p mock-treblle-api

FROM debian:bullseye-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl1.1 \
    && rm -rf /var/lib/apt/lists/*

# Copy the binary from the builder stage
COPY --from=builder /app/target/release/mock-treblle-api /usr/local/bin/

CMD ["mock-treblle-api"]
