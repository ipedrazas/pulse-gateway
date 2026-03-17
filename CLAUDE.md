# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**Pulse Gateway** — a Tauri 2.0 desktop app (macOS) that manages a Caddy-based reverse proxy for local Docker containers, providing automatic HTTPS subdomains (e.g., `app-name.andcake.dev`) via wildcard SSL with DNS-01 challenge (Cloudflare/Porkbun).

## Tech Stack

- **Frontend:** Vue 3 + TypeScript + Vite (port 1420)
- **Backend:** Rust via Tauri 2.0
- **Package manager:** bun (configured in tauri.conf.json)
- **Proxy engine:** Caddy (runs as a Docker container, managed via its JSON Admin API on port 2019)
- **Docker integration:** `bollard` crate (planned) for Docker socket monitoring

## Build & Dev Commands

```bash
# Install frontend dependencies
bun install

# Run in development mode (starts both Vite dev server and Tauri app)
bun run tauri dev

# Build for production
bun run tauri build

# Frontend only (no Tauri shell)
bun run dev        # dev server
bun run build      # vue-tsc + vite build
bun run preview    # preview production build

# Rust backend only
cd src-tauri && cargo build
cd src-tauri && cargo test
```

## Architecture

The app follows Tauri's two-process model:

- **`src/`** — Vue 3 frontend. Communicates with the Rust backend via `invoke()` from `@tauri-apps/api/core`.
- **`src-tauri/src/`** — Rust backend. Exposes Tauri commands (decorated with `#[tauri::command]`) that the frontend calls. Will use `bollard` for Docker socket events and `reqwest` for Caddy API calls.
- **`src-tauri/tauri.conf.json`** — Tauri configuration. Defines build commands, window settings, and app identifier (`dev.andcake.pulsegw`).

### Key Design Decisions (from PRD)

- Caddy routes are managed dynamically via its **JSON Admin API** (`PATCH /config/apps/http/servers/srv0/routes`), not via Caddyfile.
- Containers opt in to proxying with the Docker label `andcake.proxy=true`.
- A dedicated Docker network `andcake-gateway` connects Caddy to proxied containers.
- DNS credentials are stored locally (system keyring or encrypted app config).
