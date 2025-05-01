#!/bin/bash
export RUST_BACKTRACE=1
cargo run --bin chat "$@" --features="server-http2 chat-in auth-out"
