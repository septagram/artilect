#!/bin/bash
export RUST_BACKTRACE=1
cargo run --bin telegram "$@" --features="backend telegram-in" # auth-out"