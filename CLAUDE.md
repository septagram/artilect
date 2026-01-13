# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Artilect is a modular AI agent framework inspired by "Symphony of Thought" by David Shapiro. It implements a precept-based architecture where services (precepts) communicate via a message-passing system. The project is in early development and aims to build sophisticated AI agents with dynamic task management and external system interaction.

## Version Control

This repository uses **jj (Jujutsu)** as its version control system, not git. Use `jj` commands instead of `git`:

```bash
jj status      # instead of git status
jj diff        # instead of git diff
jj commit      # instead of git commit (note: jj commit works differently)
jj log         # instead of git log
```

## Build System & Commands

### Building Binaries

The project uses a complex feature-based build system with multiple binaries. Each binary requires specific features:

```bash
# Build specific binary with required features
cargo build --bin=<binary-name> --features="<required-features>"

# Example: Build the chat frontend for desktop
dx run --bin=chat-front --release --platform=desktop --features="chat-front chat-out"

# Example: Build the chat frontend for web
dx serve --bin=chat-front --release --platform=web --addr="0.0.0.0" --port=3000 --features="chat-front chat-out"
```

### Available Binaries

- `artilect` - Full system (requires: auth-front, auth-out, chat-front, chat-out)
- `artilect-cortex` - Backend cortex (requires: auth-in, chat-in, telegram-in)
- `artilect-minicortex` - Integrated system (requires: auth-in, auth-front, chat-in, chat-front, telegram-in)
- `auth` - Auth service (requires: auth-in, server-http2)
- `auth-front` - Auth frontend (requires: auth-front, auth-out)
- `chat` - Chat service (requires: chat-in, server-http2)
- `chat-front` - Chat frontend (requires: chat-front, chat-out)
- `telegram` - Telegram bot (requires: telegram-in, auth-out, server-http2)

### Justfile Commands

The project uses `just` for common tasks:

```bash
# Get required features for a binary
just get-required-features <binary-name>

# Run chat frontend in desktop mode
just chat-front-desktop

# Serve chat frontend on web
just chat-front-web
```

### Testing

```bash
# Run all tests
cargo test

# Run tests for specific binary with features
cargo test --bin=<binary-name> --features="<required-features>"

# Run specific test
cargo test <test-name>
```

### Database Setup

```bash
# Navigate to db directory and run migrations
cd db
./migrate.sh

# The script:
# 1. Loads DATABASE_URL from .env
# 2. Runs migrations with refinery
# 3. Exports schema to schema.sql for AI reference
# 4. Exports functions/procedures to functions.sql for AI reference
```

Both `schema.sql` and `functions.sql` are auto-generated and gitignored - they provide Claude Code with current database structure and stored procedures for context.

## Architecture

### Core Concepts

**Precepts**: Self-contained services that handle specific domains (auth, chat, telegram). Each precept can run locally (in-process via Actix actors) or remotely (via HTTP/2).

**Orchestra**: Generated orchestration layer that manages precept lifecycle and routing. Defined using the `orchestra_from_precepts!` macro in `src/orchestra.rs`.

**Dual-Mode Communication**: Precepts support both local (Actix message passing) and remote (HTTP/2) communication through a unified client interface.

### Project Structure

```
src/
├── bin/              # Executable entry points for different deployment configurations
├── precept.rs        # Core precept traits and types
├── precepts.rs       # Precept module declarations
├── precepts/
│   ├── auth/         # Authentication & authorization
│   ├── chat/         # Chat interface with LLM integration
│   └── telegram/     # Telegram bot integration
├── orchestra.rs      # Precept orchestration via macros
├── infer/            # LLM inference utilities
├── config/           # Configuration management
└── prompts/          # Prompt templates

macro/                # Procedural macros for precepts, DTOs, and orchestra
db/
├── migrations/       # Database migration files
└── schema.sql        # Current database schema (auto-generated)
```

### Precept Architecture

Each precept module contains:
- `dto.rs` - Data transfer objects and message types
- `local.rs` - Backend implementation (Actix actor)
- `remote.rs` - Remote client implementation
- `front.rs` - Frontend UI (Dioxus components)

### Message Passing

Messages between precepts use `SignedMessage<T>`:
```rust
pub struct SignedMessage<T> {
    pub from: Identity,  // Who sent the message
    pub data: T,         // The actual message
}
```

Identity can be either a User or a Precept acting on behalf of someone.

### Feature Flags

The codebase heavily uses Cargo features to enable/disable functionality:

- `*-in` features: Enable backend/server implementations
- `*-out` features: Enable client implementations
- `*-front` features: Enable frontend UI components
- `backend`: Core backend dependencies (actix, tokio, sqlx)
- `frontend`: Core frontend dependencies (dioxus)
- `infer`: LLM inference capabilities
- `server-http2`: HTTP/2 server support (axum)
- `client-http2`: HTTP/2 client support (reqwest)

### Custom Macros

The project uses extensive procedural macros from the `macro/` workspace member:

- `#[precept(...)]` - Define a precept with message handlers
- `#[precept_message]` - Define message types
- `orchestra_from_precepts!` - Generate orchestra boilerplate
- `#[dto]` - Generate DTO serialization/routing code

## Development Environment

### Required Tools

- Rust nightly toolchain (see `rust-toolchain.toml`)
- PostgreSQL database
- `refinery` for migrations
- `dx` (Dioxus CLI) for frontend development
- `just` for task running

### Environment Configuration

Create `.env` files:
- `db/.env` - DATABASE_URL for PostgreSQL
- Root `.env` - Service configuration (ports, model URLs, etc.)

### Rust Features

The project requires nightly Rust and uses unstable features:
- `str_as_str`
- `error_generic_member_access`
- `custom_inner_attributes`
- `proc_macro_hygiene`
- `const_option_ops`
- `if_let_guard` (in macros)

## Key Implementation Notes

1. **AddressBook Pattern**: The `AddressBook` (generated by `orchestra_from_precepts!`) provides typed access to all precepts with identity/token context.

2. **Client Types**: Each precept exposes different client types:
   - `ClientLocal<P>` - In-process communication via Actix
   - `ClientRemote` - Remote communication via HTTP/2
   - `Client<P>` - Enum wrapping both for flexible deployment

3. **Router Construction**: Precepts with `server-http2` feature implement `Routable` trait to expose HTTP endpoints.

4. **Database Functions**: Complex auth logic lives in PostgreSQL functions (e.g., `get_user_from_login` handles user creation/lookup atomically).

5. **LLM Integration**: The `infer` module provides prompt chain utilities and OpenAI-compatible API support for the chat precept.

## Common Patterns

### Adding a New Precept

1. Create module in `src/precepts/`
2. Define DTOs with message types
3. Implement `local.rs` with `#[precept(...)]` attribute
4. Add features to `Cargo.toml` (`name-in`, `name-out`, etc.)
5. Register in `src/orchestra.rs` via `orchestra_from_precepts!` macro
6. Add binary configurations if needed

### Working with Messages

Use the `#[precept_message]` attribute to define message handlers:
```rust
#[precept_message]
async fn handle_message(
    resources: &Resources,
    state: &State,
    from: Identity,
    message: MessageType
) -> Result<ResponseType> {
    // Implementation
}
```

### Database Queries

Use `sqlx::query_as!` for type-safe queries against the schema exported by `migrate.sh`.
