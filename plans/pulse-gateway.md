# Plan: Pulse Gateway

> Source PRD: `docs/PRD.md`

## Architectural decisions

Durable decisions that apply across all phases:

- **Proxy engine**: Caddy running as a Docker container, managed entirely via its JSON Admin API on `localhost:2019`
- **Caddy image**: Pre-built image on Docker Hub (default: `ipedrazas/pulse-caddy`) with Cloudflare + Porkbun DNS plugins baked in. User-overridable in Settings.
- **Caddy persistence**: Docker volumes (`pulse-caddy-config:/config`, `pulse-caddy-data:/data`) so routes and certificates survive container restarts
- **Caddy lifecycle**: App manages Caddy container startup but leaves it running when app quits. App is a control plane, not the data plane.
- **Caddy route management**: Rust backend holds the canonical route list and full-pushes to Caddy on every mutation via `PATCH /config/apps/http/servers/srv0/routes`
- **Docker network**: Dedicated `pulse-gateway` bridge network connecting Caddy to proxied containers
- **Container opt-out**: All containers are auto-routed by default; opt out with label `pulse.proxy=false`. The Caddy sidecar container is always excluded.
- **Label namespace**: `pulse.*` — `pulse.proxy` (opt-out), `pulse.port` (explicit port override), `pulse.subdomain` (custom subdomain)
- **Port detection priority**: (1) `pulse.port` label, (2) static route rule matching the container's image, (3) `EXPOSE` directives
- **Multi-port routing**: Containers exposing multiple ports get a route per port: `{name}-{port}.{domain}`. Static route rules (image-based) can override this with custom subdomain templates.
- **Static routes**: Image-based templates with per-port subdomain patterns (e.g., image `postgres:*`, port 5432 → `{name}-db`). Also supports manual routes (subdomain, target host, port) independent of containers.
- **Name collisions**: First-come-first-served. Duplicates get suffix `-2`, `-3`, etc. A warning is logged.
- **Startup reconciliation**: On launch, enumerate all running containers, diff against Caddy's current routes, add missing, remove stale.
- **Key models**: `Gateway` (subdomain, container name, port, status), `DnsConfig` (provider, api_key, domain), `StaticRouteRule` (image pattern, port→subdomain mappings)
- **IPC**: Tauri two-process model — Vue 3 frontend calls Rust backend via `invoke()` from `@tauri-apps/api/core`. Backend→frontend via Tauri event system (`emit`/`listen`).
- **Frontend stack**: Vue 3 + TypeScript + Pinia (state management) + vue-router
- **Credential storage**: System keyring via `keyring` crate (macOS Keychain)
- **SSL strategy**: Let's Encrypt wildcard certificate via DNS-01 challenge (Cloudflare / Porkbun)
- **Error handling**: Auto-restart Caddy container on crash. Error state + guidance for Docker-not-running or port conflicts.
- **Branding**: Bundle ID is `dev.andcake.pulsegw`. All other references use `pulse-*` naming (network, labels, container names).

---

## Phase 1: Caddy Sidecar + First Route

**User stories**: §4.1 Caddy Sidecar, §4.2 Caddy API Integration, §4.3 Network Logic

### What to build

The Rust backend manages the full lifecycle of a Caddy Docker container: pull/ensure the image (pre-built from Docker Hub, configurable in Settings), create the `pulse-gateway` network, start Caddy attached to that network with persistent volumes, and expose port 2019 (admin) and 443 (HTTPS). Tauri commands allow adding and removing reverse proxy routes via the Caddy Admin API. Additionally, users can define **static routes** (subdomain, target host/IP, port) through the UI — these are persisted in app config and pushed to Caddy on every startup, independent of any running container. The Vue frontend shows Caddy status, a domain configuration field, and a form to manage static routes. The frontend uses Pinia for state management and vue-router for navigation.

### Acceptance criteria

- [ ] App starts/ensures a Caddy container on the `pulse-gateway` Docker network with persistent volumes
- [ ] Caddy Admin API is reachable at `localhost:2019` from the Rust backend
- [ ] Settings page with domain input field and Caddy image override
- [ ] A Tauri command accepts (subdomain, target_host, port) and creates a reverse proxy route in Caddy for `{subdomain}.{domain}`
- [ ] A corresponding Tauri command removes a route by subdomain
- [ ] Rust backend maintains the canonical route list and full-pushes to Caddy on every change
- [ ] Static routes are persisted to local app config and restored on app startup
- [ ] Frontend displays Caddy connection status and a form to add/remove static routes
- [ ] Frontend uses Pinia for state management and vue-router for navigation (Dashboard + Settings views)
- [ ] `bollard` and `reqwest` crates are integrated into the Rust backend
- [ ] Caddy container keeps running when the app quits
- [ ] Error state with guidance when Docker is not running or Caddy fails to start (port conflict)

---

## Phase 2: Docker Event Listener + Auto-Routing

**User stories**: §3.1 Auto-Detection, Dynamic Routing, Cleanup

### What to build

A background task in the Rust backend subscribes to Docker socket events via `bollard`. When any container starts, the backend extracts its name and exposed ports, attaches it to the `pulse-gateway` network if needed, and pushes new routes to Caddy. Containers with the label `pulse.proxy=false` are skipped, as is the Caddy sidecar itself. Multi-port containers get one route per port (`{name}-{port}.{domain}`), unless an image-based static route rule defines custom mappings. Name collisions are resolved by appending `-2`, `-3`, etc. When a container stops or dies, the corresponding routes are removed. On app startup, all running containers are reconciled against Caddy's current routes. The frontend receives real-time updates via Tauri events and reflects the current set of active gateways.

### Acceptance criteria

- [ ] Backend listens to Docker socket events in a long-running background task
- [ ] Any container start triggers automatic Caddy route creation within 2 seconds
- [ ] Containers with label `pulse.proxy=false` are excluded from auto-routing
- [ ] The Caddy sidecar container is always excluded from auto-routing
- [ ] Container stop/die triggers automatic Caddy route removal
- [ ] Containers are auto-attached to `pulse-gateway` network if not already connected
- [ ] Port detection follows priority: `pulse.port` label > static route rule for image > `EXPOSE` directives
- [ ] Multi-port containers get one route per port: `{name}-{port}.{domain}`
- [ ] Image-based static route rules override default multi-port naming
- [ ] Name collisions resolved: first-come-first-served, duplicates get `-2`, `-3` suffix with a logged warning
- [ ] On app startup, reconcile running containers against Caddy routes (add missing, remove stale)
- [ ] Auto-restart Caddy container if it crashes
- [ ] Frontend updates in real time via Tauri events as gateways are added/removed

---

## Phase 3: SSL Wildcard + Credential Management

**User stories**: §3.2 Wildcard Provisioning, Credential Management, Health Checks

### What to build

The Settings page is extended with DNS provider selection (Cloudflare or Porkbun) and API credential fields. The backend stores credentials securely in the system keyring via the `keyring` crate and configures Caddy's TLS automation policy to use DNS-01 challenge for `*.{domain}`. The app monitors the certificate status and surfaces expiration info to the frontend.

### Acceptance criteria

- [ ] Settings UI extended with DNS provider selector and API key fields
- [ ] Credentials stored securely in system keyring via `keyring` crate
- [ ] Backend configures Caddy's TLS automation for wildcard DNS-01 challenge using stored credentials
- [ ] Wildcard certificate is successfully provisioned on first setup
- [ ] Certificate expiration date is retrieved and displayed in the UI
- [ ] Settings persist across app restarts

---

## Phase 4: Dashboard + Logs

**User stories**: §3.3 Main View, Logs; §5 Success Criteria

### What to build

The full dashboard experience: an "Active Gateways" list showing each proxied container's subdomain, target container/port, and status (Secure / Proxying / Error). A live event log streams Docker listener events (container detected, route added, route removed, errors). The UI ties together all prior phases into a polished, usable interface.

### Acceptance criteria

- [ ] Active Gateways list displays subdomain, target container, port, and status for each route
- [ ] Status indicators distinguish between Secure (HTTPS working), Proxying (HTTP only), and Error states
- [ ] Live event log shows timestamped entries for container detection, route changes, and errors
- [ ] Clicking a gateway's subdomain opens it in the default browser
- [ ] Dashboard correctly reflects state after app restart (re-scans running containers)
- [ ] Zero-config routing: start a container → HTTPS subdomain available within 2 seconds
