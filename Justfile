#!/usr/bin/env just --justfile
set shell := ["C:/Program Files/Git/bin/bash.exe", "-c"]

# Extract required features for a binary from Cargo.toml
get-required-features bin:
    @yq eval '.bin[] | select(.name == "{{bin}}") | .required-features | join(" ")' Cargo.toml -o=toml

# Auth Service (usage: just auth [run|build])
auth action="run":
    cargo {{action}} --bin=auth --features="$(just get-required-features auth)"

# Auth Frontend (usage: just auth-front [run|build])
auth-front action="run":
    dx {{action}} --bin=auth-front --release --platform=desktop --features="$(just get-required-features auth-front)"

# Chat Frontend (usage: just chat-front [run|build])
chat-front action="run":
    dx {{action}} --bin=chat-front --release --platform=desktop --features="$(just get-required-features chat-front)"

# Dev Proxy (usage: just proxy [run|start|stop|reload])
proxy action="run":
    caddy {{action}} --config Caddyfile

# Dev Environment Help
dev-help:
    @echo "Artilect Dev Environment"
    @echo ""
    @echo "Services:"
    @echo "  - Auth Service:  just auth        (run)"
    @echo "                   just auth build  (build only)"
    @echo ""
    @echo "Frontends:"
    @echo "  - Auth Frontend: just auth-front"
    @echo "  - Chat Frontend: just chat-front"
    @echo ""
    @echo "Proxy:"
    @echo "  - just proxy          (run in foreground)"
    @echo "  - just proxy start    (run in background)"
    @echo "  - just proxy stop"
    @echo "  - just proxy reload"
    @echo ""
    @echo "Access via proxy (http://localhost:8080):"
    @echo "  - Auth: /auth, /auth/api"
    @echo "  - Chat: /chat, /chat/api"
