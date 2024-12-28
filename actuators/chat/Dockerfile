FROM rust:1.75 as builder

WORKDIR /usr/src/chat
COPY . .

RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/chat/target/release/chat /usr/local/bin/chat

EXPOSE 8080

CMD ["chat"]
