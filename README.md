# Pulse Gateway

A macOS desktop app that automatically routes your local Docker containers through a Caddy reverse proxy with wildcard HTTPS subdomains.

Start a container, get `container-name.yourdomain.dev` with a valid SSL certificate — no config needed.

## How It Works

Pulse Gateway watches the Docker socket for container events. When a container starts, it automatically:

1. Attaches the container to a shared Docker network (`pulse-gateway`)
2. Generates a subdomain from the container name (sanitized for DNS)
3. Pushes an updated route config to Caddy's Admin API
4. Caddy serves the container over HTTPS using a wildcard certificate

When a container stops, its route is removed automatically.

## Features

- **Auto-routing** — all containers are proxied by default (opt out with `pulse.proxy=false` label)
- **Wildcard SSL** — single `*.yourdomain.dev` certificate via DNS-01 challenge
- **DNS providers** — Cloudflare and Porkbun supported
- **Credential storage** — API tokens stored in macOS Keychain
- **Static routes** — manually define routes to non-Docker services
- **Port detection** — `pulse.port` label > image-based route rules > `EXPOSE` directives
- **Real-time dashboard** — live container status, route table, and event log
- **Start/stop Caddy** — manage the Caddy container directly from the app

## Prerequisites

- macOS
- [Docker Desktop](https://www.docker.com/products/docker-desktop/)
- [Bun](https://bun.sh/) (package manager)
- [Rust](https://rustup.rs/) (for building the Tauri backend)
- A domain with DNS managed by Cloudflare or Porkbun

## Quick Start

```bash
# Install dependencies
bun install

# Run in development mode
bun run tauri dev
```

On first launch:

1. Go to **Settings** and set your **Root Domain** (e.g. `onlocalhost.xyz`)
2. Select your **DNS Provider** (Cloudflare or Porkbun)
3. Set the **Caddy Docker Image** to one with your DNS plugin:
   - Cloudflare: `ghcr.io/caddybuilds/caddy-cloudflare:latest`
   - Porkbun: `ghcr.io/caddy-dns/porkbun:latest`
4. Add the required **environment variables**:
   - Cloudflare: `CLOUDFLARE_API_TOKEN`
   - Porkbun: `PORKBUN_API_KEY` and `PORKBUN_API_SECRET`
5. Go to **Dashboard** and click **Start Caddy**

Any running Docker container will now be accessible at `container-name.yourdomain.dev`.

## Container Labels

Control routing behavior with Docker labels:

| Label | Description | Example |
|-------|-------------|---------|
| `pulse.proxy` | Set to `false` to exclude a container | `pulse.proxy=false` |
| `pulse.port` | Override the port to proxy | `pulse.port=8080` |
| `pulse.subdomain` | Override the subdomain name | `pulse.subdomain=myapp` |

```bash
# Example: run a container with a custom subdomain and port
docker run -d --label pulse.subdomain=api --label pulse.port=3000 my-api-server
# → https://api.yourdomain.dev
```

## Build

```bash
# Production build
bun run tauri build

# Frontend only
bun run build

# Rust backend only
cd src-tauri && cargo build
```

## Tech Stack

- **Frontend:** Vue 3 + TypeScript + Vite + Pinia + vue-router
- **Backend:** Rust via Tauri 2.0
- **Proxy:** Caddy (Docker container, managed via JSON Admin API)
- **Docker:** `bollard` crate for socket monitoring
- **Credentials:** macOS Keychain via `keyring` crate (with `apple-native` feature)

## Architecture

```
┌─────────────────────────────────────────────┐
│  Pulse Gateway (Tauri app)                  │
│  ┌──────────┐  invoke()  ┌───────────────┐  │
│  │ Vue 3 UI │ ◄────────► │ Rust backend  │  │
│  └──────────┘   events   └───────┬───────┘  │
│                                  │          │
│                    Docker socket ─┤         │
│                    Caddy API ─────┤         │
│                    Keychain ──────┘         │
└─────────────────────────────────────────────┘
         │                    │
         ▼                    ▼
┌─────────────┐    ┌──────────────────┐
│ Caddy       │◄──►│ Your containers  │
│ (Docker)    │    │ (Docker)         │
│ :443 :80    │    │                  │
└─────────────┘    └──────────────────┘
    pulse-gateway network (bridge)
```

## License

MIT
