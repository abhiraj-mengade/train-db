FROM rust:1.75 as builder

WORKDIR /usr/src/app
COPY . .
RUN cargo build --release --features bluetooth

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/app/target/release/rpc-node /usr/local/bin/rpc-node

EXPOSE 8546

CMD ["rpc-node"]
