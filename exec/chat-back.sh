#!/bin/bash
cargo run --bin chat "$@" --features="server-http2 chat-in auth-out"
