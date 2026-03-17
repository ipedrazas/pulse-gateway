# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**Pulse Gateway** — a Tauri 2.0 desktop app (macOS) that manages a Caddy-based reverse proxy for local Docker containers, providing automatic HTTPS subdomains (e.g., `app-name.yourdomain.dev`) via wildcard SSL with DNS-01 challenge (Cloudflare/Porkbun). Uses an opt-out model: all containers are auto-routed unless labeled `pulse.proxy=false`.

## Tech Stack

- **Frontend:** Vue 3 + TypeScript + Vite (port 1420) + Pinia (state) + vue-router
- **Backend:** Rust via Tauri 2.0
- **Package manager:** bun (configured in tauri.conf.json)
- **Proxy engine:** Caddy (runs as a Docker container, managed via its JSON Admin API on port 2019)
- **Docker integration:** `bollard` crate for Docker socket monitoring
- **Credential storage:** System keyring via `keyring` crate

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

- **`src/`** — Vue 3 frontend. Communicates with the Rust backend via `invoke()` from `@tauri-apps/api/core`. Receives real-time updates via Tauri event system (`listen`).
- **`src-tauri/src/`** — Rust backend. Exposes Tauri commands (decorated with `#[tauri::command]`) that the frontend calls. Uses `bollard` for Docker socket events, `reqwest` for Caddy API calls, and Tauri `emit` for backend→frontend events.
- **`src-tauri/tauri.conf.json`** — Tauri configuration. Defines build commands, window settings, and app identifier (`dev.andcake.pulsegw`).

### Key Design Decisions

- **Opt-out routing**: All containers are auto-routed by default. Opt out with `pulse.proxy=false`. Caddy sidecar is always excluded.
- **Label namespace**: `pulse.*` — `pulse.proxy`, `pulse.port`, `pulse.subdomain`.
- **Caddy routes** are managed dynamically via its JSON Admin API (`PATCH /config/apps/http/servers/srv0/routes`). Rust backend holds the canonical route list and full-pushes on every mutation.
- **Docker network**: `pulse-gateway` bridge network connects Caddy to proxied containers.
- **Caddy lifecycle**: App starts Caddy but leaves it running on quit. Persistent volumes for config and certs.
- **Port detection**: `pulse.port` label > image-based static route rule > `EXPOSE` directives.
- **Multi-port**: One route per port as `{name}-{port}.{domain}`, overridable by static route rules.
- **Name collisions**: First-come-first-served, duplicates get `-2`, `-3` suffix.
- **Startup reconciliation**: On launch, diff running containers against Caddy routes.
- **Credentials**: System keyring via `keyring` crate (macOS Keychain).
- **Error handling**: Auto-restart Caddy on crash. Error state + guidance for Docker-down or port conflicts.
