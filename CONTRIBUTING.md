# Contributing to Pulse Gateway

Thanks for your interest in contributing! This guide will help you get set up and familiar with the project.

## Prerequisites

- **macOS** (the app is macOS-only for now)
- [Docker Desktop](https://www.docker.com/products/docker-desktop/)
- [Rust](https://rustup.rs/)
- [Bun](https://bun.sh/)
- [Task](https://taskfile.dev/) (optional, but recommended)

## Getting Started

```bash
# Clone the repo
git clone https://github.com/ipedrazas/pulse-gateway.git
cd pulse-gateway

# Install frontend dependencies
bun install

# Run in development mode
task dev
# or: bun run tauri dev
```

This starts both the Vite dev server (port 1420) and the Tauri app. The app will hot-reload on frontend changes; backend (Rust) changes trigger a recompile.

## Project Structure

```
src/                    # Vue 3 frontend (TypeScript)
  views/                # Page components (Dashboard, Settings)
  stores/               # Pinia stores (gateway, settings)
  router/               # Vue Router config
  assets/               # Static assets (SVG logo)
src-tauri/
  src/                  # Rust backend
    lib.rs              # App setup, state, command registration
    commands.rs         # Tauri commands (frontend ↔ backend)
    caddy.rs            # Caddy API client + config builder
    docker.rs           # Docker socket operations (bollard)
    watcher.rs          # Docker event listener + auto-routing
    credentials.rs      # Keychain/file credential storage
    config.rs           # App config persistence
    models.rs           # Shared data types
  tauri.conf.json       # Tauri app configuration
```

## Common Tasks

All tasks are defined in `Taskfile.yaml`. Run `task --list` to see them all.

```bash
task dev              # Run in dev mode
task build            # Production build
task check            # cargo check + vue-tsc
task test             # Run all tests
task lint             # cargo clippy
task fmt              # Format all code (cargo fmt + prettier)
task clean            # Remove build artifacts
```

## Code Style

### Frontend (TypeScript/Vue)

- Formatted with **Prettier** (`bun run format`)
- Type-checked with **vue-tsc** (`bun run build` or `task check:frontend`)
- Double quotes, semicolons, trailing commas

### Backend (Rust)

- Formatted with **cargo fmt** (`task fmt:backend`)
- Linted with **cargo clippy** (`task lint`)
- All clippy warnings must be resolved before merging

### Before Submitting

Run the full check suite:

```bash
task fmt
task check
task lint
```

## Architecture Overview

The app follows Tauri's two-process model:

- **Frontend** communicates with the backend via `invoke()` from `@tauri-apps/api/core`
- **Backend** exposes `#[tauri::command]` functions and pushes real-time updates via `emit()`
- **Caddy** runs as a Docker container, managed via its JSON Admin API (port 2019)
- **Docker events** are monitored via the Docker socket using the `bollard` crate

### Key flows

1. **Container starts** → watcher detects event → inspects container → generates subdomain → pushes route to Caddy → emits `gateways-changed` to frontend
2. **Container stops** → watcher removes route → pushes updated config to Caddy
3. **User adds static route** → frontend calls `add_route` command → backend saves to config + pushes to Caddy
4. **Caddy restarts** → watcher detects event → re-pushes all routes

### Docker labels

Containers can use `pulse.*` labels to control routing:

| Label | Effect |
|-------|--------|
| `pulse.proxy=false` | Exclude from auto-routing |
| `pulse.port=8080` | Override the proxied port |
| `pulse.subdomain=myapp` | Override the subdomain name |

## Submitting Changes

1. Fork the repo and create a branch from `main`
2. Make your changes
3. Run `task fmt && task check && task lint`
4. Open a pull request with a clear description of what changed and why

## Reporting Issues

Open an issue at [github.com/ipedrazas/pulse-gateway/issues](https://github.com/ipedrazas/pulse-gateway/issues) with:

- Steps to reproduce
- Expected vs actual behavior
- App version (shown in Settings)
- macOS version
