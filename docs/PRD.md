# PRD: Pulse Gateway

## 1. Product Vision
A developer-centric desktop utility to automate the "Local-to-SSL" workflow. The app manages a Caddy-based proxy that monitors the local Docker environment to provide instant, secure subdomains (e.g., `app-name.yourdomain.dev`) for any container.

## 2. Technical Foundation
*   **App Framework:** Tauri 2.0 (Rust backend, Web-based frontend).
*   **Frontend:** Vue 3 + TypeScript + Pinia + vue-router.
*   **Proxy Engine:** **Caddy** (Running as a Docker container, pre-built image on Docker Hub with DNS plugins).
*   **Docker Integration:** `bollard` crate (Rust) for Docker Socket events.
*   **SSL Strategy:** Let's Encrypt Wildcard via **DNS-01 Challenge**.
*   **DNS Providers:** Initial support for Cloudflare and Porkbun.
*   **Credential Storage:** System keyring via `keyring` crate (macOS Keychain).

## 3. Functional Requirements

### 3.1 Docker Orchestration (The "Worker")
*   **Auto-Detection:** The app must listen to the Docker Socket (`/var/run/docker.sock`) for container events.
*   **Dynamic Routing (opt-out model):** When any container starts, it is auto-routed unless it has the label `pulse.proxy=false`:
    *   Extract the container name (to use as subdomain).
    *   Identify the internal port: `pulse.port` label > image-based static route rule > `EXPOSE` directive.
    *   For multi-port containers: create one route per port as `{name}-{port}.{domain}`, unless an image-based static route rule defines custom mappings.
    *   Push new route(s) to Caddy's Admin API (`:2019`).
    *   The Caddy sidecar container is always excluded.
*   **Name Collisions:** First-come-first-served. Duplicates get suffix `-2`, `-3`, etc. with a logged warning.
*   **Cleanup:** When a container stops/dies, immediately remove the corresponding route(s) from Caddy.
*   **Startup Reconciliation:** On app launch, enumerate all running containers and reconcile against Caddy's current routes.

### 3.2 SSL & DNS Management
*   **Wildcard Provisioning:** On first setup, the app instructs Caddy to request a certificate for `*.yourdomain.dev`.
*   **Credential Management:** Securely store Cloudflare API Tokens or Porkbun API Keys in the system keyring.
*   **Health Checks:** Monitor the SSL certificate status and display the expiration date in the UI.

### 3.3 The Dashboard (UI)
*   **Main View:** A list of "Active Gateways" showing:
    *   Subdomain (e.g., `demo.yourdomain.dev`)
    *   Target Container & Port.
    *   Status (Secure/Proxying/Error).
*   **Configuration (Settings page):**
    *   Root Domain input.
    *   Caddy Docker image override.
    *   DNS Provider selection.
    *   API Key inputs.
*   **Logs:** A stream of events from the Docker listener (e.g., "Detected new container: 'postgres-ui', mapping to 'postgres-ui.yourdomain.dev'").

## 4. Technical Architecture Detail

### 4.1 Caddy Sidecar
The app will ensure a specific Caddy image is running (default: pre-built image on Docker Hub with `caddy-dns/cloudflare` and `caddy-dns/porkbun`). The image is configurable in Settings. Docker volumes (`pulse-caddy-config:/config`, `pulse-caddy-data:/data`) persist Caddy's config and certificates across restarts. The Caddy container keeps running when the app quits — the app is a control plane, not the data plane.

### 4.2 Caddy API Integration
Instead of managing a physical `Caddyfile`, the Tauri backend will interact with Caddy's **JSON API**.
*   **Endpoint:** `PATCH /config/apps/http/servers/srv0/routes`
*   **Payload:** A JSON object defining the host matcher and the reverse_proxy upstream.
*   The Rust backend maintains the canonical route list and full-pushes to Caddy on every mutation.

### 4.3 Network Logic
To allow Caddy to route traffic to other containers, the app will:
1. Create a dedicated Docker network: `pulse-gateway`.
2. Automatically attach the Caddy container to this network.
3. Auto-attach proxied containers to `pulse-gateway` if not already connected.

### 4.4 Static Route Rules
Users can define image-based routing templates:
*   Keyed on Docker image name/pattern (e.g., `postgres:*`).
*   Per-port subdomain patterns (e.g., port 5432 → `{name}-db`, port 8080 → `{name}-ui`).
*   When a container matches an image rule, the rule's mappings are used instead of the default multi-port naming.
*   Also supports manual static routes (subdomain, target host/IP, port) independent of any container.

### 4.5 Error Handling
*   **Docker not running:** Error state with guidance ("Start Docker Desktop"), manual retry button.
*   **Caddy crash:** Auto-restart the Caddy container.
*   **Port conflict:** Error state with guidance ("Port 443 is in use").

## 5. Success Criteria (MVP)
1.  **Zero-Config Routing:** A user starts a container, and within 2 seconds, it is accessible via HTTPS on their custom domain.
2.  **Wildcard SSL:** One single certificate handles all future subdomains without further DNS updates.
3.  **Persistence:** API keys, domain settings, and Caddy config survive app restarts.
