FROM rust:1.75 as builder

WORKDIR /usr/src/web-chat
COPY . .

RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/web-chat/target/release/web-chat /usr/local/bin/web-chat

EXPOSE 8080

CMD ["web-chat"] 