My humble shot at AGI.

# Artilect: A Modular AI Agent Framework

Artilect is a project aimed at building a modular, extensible, and intelligent framework for constructing sophisticated AI agents capable of dynamic task management, proactive user interaction and interaction with external systems. It’s inspired by architectural principles outlined in the book ["Symphony of Thought"](https://www.barnesandnoble.com/w/symphony-of-thought-david-shapiro/1142248298) by David Shapiro.

It's currently a work in progress in a very early stage.

## Local Development Setup

### 1. Database Setup

1. Create a PostgreSQL database
2. Create `.env` file in `db/` directory:
   ```env
   DATABASE_URL=postgres://user:password@localhost:5432/artilect
   ```
3. Run migrations:
   ```bash
   cd db
   ./migrate.sh
   ```

### 2. Chat Backend

1. Create `.env` file in `actuators/chat/`:
   ```env
   PORT=3001
   INFER_URL=http://localhost:11000
   DEFAULT_MODEL=mistral-instruct-0.2
   ```
2. Run the service:
   ```bash
   cd actuators/chat
   cargo run
   ```

### 3. Chat Frontend

Run in web mode:

```bash
cd actuators/chat-front
dx serve --platform=web [--port=1234]
```

Or desktop mode:

```bash
cd actuators/chat-front
dx serve --platform=desktop
```
