#!/usr/bin/env just --justfile
set shell := ["C:/Program Files/Git/bin/bash.exe", "-c"]

# Extract required features for a binary from Cargo.toml
get-required-features bin:
    @yq eval '.bin[] | select(.name == "{{bin}}") | .required-features | join(" ")' Cargo.toml -o=toml

chat-front-desktop:
    dx run --bin=chat-front --release --platform=desktop --features="$(just get-required-features chat-front)"

chat-front-web:
    dx serve --bin=chat-front --release --platform=web --addr="0.0.0.0" --port=3000 --features="$(just get-required-features chat-front)"
