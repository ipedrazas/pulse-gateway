# Changelog

All notable changes to Pulse Gateway are documented here.

## [0.1.1] - 2026-03-20

### Added
- **DNS provider selection** — support for Cloudflare and Porkbun DNS-01 challenge providers, configurable in Settings
- **Stop Caddy** — button on Dashboard to stop the Caddy container
- **Localhost routing** — static routes targeting `localhost` are automatically translated to `host.docker.internal` for Docker networking
- **Subdomain sanitization** — container names with underscores and invalid DNS characters are cleaned up (e.g. `affectionate_ride` becomes `affectionate-ride`)
- **Certificate status** — Settings page shows issuer, SANs, and cert validity; Dashboard shows SSL/Proxy status per route
- **Version display** — app version shown at the bottom of the Settings page
- **GitHub Actions release workflow** — manual workflow to build DMG and create GitHub releases
- **Prettier** — frontend code formatting with Prettier
- **Taskfile** — `Taskfile.yaml` for common dev commands (`task dev`, `task build`, `task fmt`, etc.)
- **Contributing guide** — `CONTRIBUTING.md` for new contributors

### Fixed
- **Keychain access** — `keyring` crate now uses `apple-native` feature for reliable macOS Keychain read/write
- **Docker not running** — friendly error message when Docker Desktop is not started
- **Certificate detection** — uses `reqwest` HTTPS probe instead of shelling out to `openssl`, works in both dev and production builds
- **Race condition on startup** — cert status re-checks when gateways arrive after reconciliation
- **Per-route TLS** — Caddy automation policy covers all routed subdomains with DNS-01 instead of attempting HTTP-01

## [0.1.0] - 2026-03-18

### Added
- **Caddy sidecar** — manages a Caddy Docker container as the reverse proxy, with persistent volumes for config and certificates
- **Auto-routing** — monitors Docker socket for container start/stop events, automatically creates/removes routes
- **Opt-out model** — all containers are proxied by default; opt out with `pulse.proxy=false` label
- **Label support** — `pulse.port`, `pulse.subdomain` labels for overriding port and subdomain
- **Static routes** — manually define routes via the Dashboard UI
- **Route rules** — image-based port mapping rules for multi-port containers
- **Credential storage** — API tokens stored in macOS Keychain with file fallback
- **Dashboard** — live view of active gateways, Caddy status, and event log
- **Settings** — configure root domain, Caddy image, and environment variables
- **Startup reconciliation** — on launch, syncs running containers with Caddy routes
- **Name collision handling** — duplicate subdomains get `-2`, `-3` suffixes
