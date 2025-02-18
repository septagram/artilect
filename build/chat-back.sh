#!/bin/bash
cargo build --bin chat "$@" --features="server chat-in auth-out"
