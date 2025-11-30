#!/usr/bin/env just --justfile
set shell := ["C:/Program Files/Git/bin/bash.exe", "-c"]

# Extract required features for a binary from Cargo.toml
get-required-features bin:
    @yq eval '.bin[] | select(.name == "{{bin}}") | .required-features | join(" ")' Cargo.toml -o=toml

# Auth Frontend
auth-front-desktop:
    dx run --bin=auth-front --release --platform=desktop --features="$(just get-required-features auth-front)"

# Chat Frontend
chat-front-desktop:
    dx run --bin=chat-front --release --platform=desktop --features="$(just get-required-features chat-front)"

# Dev Proxy
proxy:
    caddy run --config Caddyfile

proxy-start:
    caddy start --config Caddyfile

proxy-stop:
    caddy stop

proxy-reload:
    caddy reload --config Caddyfile

# Dev Environment Help
dev-help:
    @echo "Artilect Dev Environment"
    @echo ""
    @echo "Services (launch from RustRover Cargo panel or cargo run):"
    @echo "  - Auth Service:  cargo run --bin auth --features auth-in,server-http2"
    @echo "  - Chat Service:  cargo run --bin chat --features chat-in,server-http2"
    @echo ""
    @echo "Frontends:"
    @echo "  - Auth Frontend: just auth-front-web  (http://localhost:5001)"
    @echo "  - Chat Frontend: just chat-front-web  (http://localhost:3000)"
    @echo ""
    @echo "Proxy:"
    @echo "  - Start: just proxy  (http://localhost:8080)"
    @echo ""
    @echo "Access via proxy:"
    @echo "  - Auth: http://localhost:8080/auth"
    @echo "  - Chat: http://localhost:8080/chat"
    @echo "  - Auth API: http://localhost:8080/auth/api"
    @echo "  - Chat API: http://localhost:8080/chat/api"
