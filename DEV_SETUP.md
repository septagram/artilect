# Artilect Development Setup

This guide explains how to run the Artilect development environment with the unified proxy setup.

## Overview

The development environment consists of:
- **Backend services** (auth, chat) running on separate ports
- **Frontend applications** (auth-front, chat-front) served by Dioxus dev server
- **Caddy reverse proxy** unifying everything under `localhost:8080`

## Why Use the Proxy?

Running services on different ports causes cookie issues - browsers treat different ports as different origins. The Caddy proxy solves this by serving everything from a single origin (`localhost:8080`), enabling cookies to work correctly across all services.

## Quick Start

### Prerequisites

- Rust toolchain (nightly)
- Caddy server (v2.10+)
- Just command runner
- Dioxus CLI (`dx`)
- PostgreSQL database

### Running the Dev Environment

**Step 1: Start Backend Services**

Launch from RustRover's Cargo panel or via terminal:

```bash
# Terminal 1: Auth Service
cargo run --bin auth --features auth-in,server-http2

# Terminal 2: Chat Service
cargo run --bin chat --features chat-in,server-http2
```

**Step 2: Start Frontend Applications**

```bash
# Terminal 3: Auth Frontend (port 5001)
just auth-front-web

# Terminal 4: Chat Frontend (port 3000)
just chat-front-web
```

**Step 3: Start Caddy Proxy**

```bash
# Terminal 5: Proxy (foreground, with logs)
just proxy

# OR run in background:
just proxy-start
```

**Step 4: Access the Application**

Open your browser to:
- **Auth Frontend**: http://localhost:8080/auth
- **Chat Frontend**: http://localhost:8080/chat

API endpoints are available at:
- **Auth API**: http://localhost:8080/auth/api/*
- **Chat API**: http://localhost:8080/chat/api/*

## Port Mapping

| Service | Direct Port | Proxy Path |
|---------|-------------|------------|
| Auth Service | 3001 | /auth/api/* |
| Chat Service | 3002 | /chat/api/* |
| Auth Frontend | 5001 | /auth/* |
| Chat Frontend | 3000 | /chat/* |
| Caddy Proxy | 8080 | / |

## Justfile Commands

```bash
# Frontend commands
just auth-front-web       # Start auth frontend
just auth-front-desktop   # Run auth frontend as desktop app
just chat-front-web       # Start chat frontend
just chat-front-desktop   # Run chat frontend as desktop app

# Proxy commands
just proxy                # Run Caddy in foreground (with logs)
just proxy-start          # Start Caddy in background
just proxy-stop           # Stop background Caddy
just proxy-reload         # Reload Caddy config without downtime

# Help
just dev-help            # Show dev environment info
```

## How the Proxy Works

The `Caddyfile` configuration:

1. **API Routes** (`/auth/api`, `/chat/api`):
   - Strips the service prefix before proxying
   - Example: `localhost:8080/auth/api/login` → `192.168.31.11:3001/login`

2. **Frontend Routes** (`/auth`, `/chat`):
   - Proxies directly to local dev servers
   - Preserves hot-reload functionality

3. **Headers**:
   - Forwards `Host`, `X-Real-IP`, `X-Forwarded-For`, `X-Forwarded-Proto`
   - Enables proper request tracking

## Troubleshooting

### Caddy won't start

```bash
# Check if port 8080 is in use
netstat -ano | findstr :8080

# Verify Caddy config
caddy validate --config Caddyfile
```

### Frontend hot-reload not working

- Ensure you're accessing via the proxy URL (`localhost:8080`)
- Check that the frontend dev server is running on the correct port
- Dioxus hot-reload should work through the proxy

### Services can't connect

- Verify all services are running with `netstat -ano | findstr :3001`
- Check the `.env` file has correct `AUTH_BASE_URL` and `CHAT_BASE_URL`
- Ensure Windows Firewall allows local connections

### Cookies still not working

- Cookies will only work once the cookie storage implementation is complete
- The proxy provides the **foundation** (unified origin) but doesn't implement cookie handling itself
- Verify all requests go through the proxy, not direct to service ports

## Development Workflow

1. Keep backend services running in RustRover
2. Launch frontends via justfile commands
3. Start Caddy proxy
4. Access everything through `localhost:8080`
5. Make code changes - frontends hot-reload automatically
6. Backend changes require service restart

## Next Steps

After setting up the proxy:
- Implement cookie storage system (keyring-based for native, browser-managed for WASM)
- Configure services to expect requests from the proxy
- Update frontend API calls to use proxy paths (`/auth/api`, `/chat/api`)
