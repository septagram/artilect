#!/bin/bash
cargo build --bin chat "$@" --features="server-http2 chat-in auth-out"
