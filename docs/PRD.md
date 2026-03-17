# PRD: Pulse Gateway

## 1. Product Vision
A developer-centric desktop utility to automate the "Local-to-SSL" workflow. The app manages a Caddy-based proxy that monitors the local Docker environment to provide instant, secure subdomains (e.g., `app-name.andcake.dev`) for any container.

## 2. Technical Foundation
*   **App Framework:** Tauri 2.0 (Rust backend, Web-based frontend).
*   **Proxy Engine:** **Caddy** (Running as a Docker container).
*   **Docker Integration:** `bollard` crate (Rust) for Docker Socket events.
*   **SSL Strategy:** Let's Encrypt Wildcard via **DNS-01 Challenge**.
*   **DNS Providers:** Initial support for Cloudflare and Porkbun.

## 3. Functional Requirements

### 3.1 Docker Orchestration (The "Worker")
*   **Auto-Detection:** The app must listen to the Docker Socket (`/var/run/docker.sock`) for container events.
*   **Dynamic Routing:** When a container starts with the label `andcake.proxy=true`:
    *   Extract the container name (to use as subdomain).
    *   Identify the internal port (from `EXPOSE` or a specific label).
    *   Push a new route configuration to Caddy's Admin API (`:2019`).
*   **Cleanup:** When a container stops/dies, immediately remove the corresponding route from Caddy.

### 3.2 SSL & DNS Management
*   **Wildcard Provisioning:** On first setup, the app instructs Caddy to request a certificate for `*.yourdomain.dev`.
*   **Credential Management:** Securely store Cloudflare API Tokens or Porkbun API Keys in the local system keyring (or encrypted app config).
*   **Health Checks:** Monitor the SSL certificate status and display the expiration date in the UI.

### 3.3 The Dashboard (UI)
*   **Main View:** A list of "Active Gateways" showing:
    *   Subdomain (e.g., `demo.andcake.dev`)
    *   Target Container & Port.
    *   Status (Secure/Proxying/Error).
*   **Configuration:**
    *   Root Domain input (e.g., `andcake.dev`).
    *   DNS Provider selection.
    *   API Key inputs.
*   **Logs:** A stream of events from the Docker listener (e.g., "Detected new container: 'postgres-ui', mapping to 'postgres-ui.andcake.dev'").

## 4. Technical Architecture Detail

### 4.1 Caddy Sidecar
The app will ensure a specific Caddy image is running. 
> **Note:** Official Caddy images don't include DNS plugins by default. The app should either build a custom image using `xcaddy` or pull a pre-built image containing `caddy-dns/cloudflare` and `caddy-dns/porkbun`.

### 4.2 Caddy API Integration
Instead of managing a physical `Caddyfile`, the Tauri backend will interact with Caddy’s **JSON API**.
*   **Endpoint:** `PATCH /config/apps/http/servers/srv0/routes`
*   **Payload:** A JSON object defining the host matcher and the reverse_proxy upstream.

### 4.3 Network Logic
To allow Caddy to route traffic to other containers, the app will:
1. Create a dedicated Docker network: `andcake-gateway`.
2. Automatically attach the Caddy container to this network.
3. Instruction for the user: "Ensure your containers are on the `andcake-gateway` network" (or automate the attachment via the app).

## 5. Success Criteria (MVP)
1.  **Zero-Config Routing:** A user starts a container with a label, and within 2 seconds, it is accessible via HTTPS on their custom domain.
2.  **Wildcard SSL:** One single certificate handles all future subdomains without further DNS updates.
3.  **Persistence:** API keys and custom domain settings survive app restarts.

---

### Implementation "Hook" for your AI Prompt:
When you ask an AI to start coding this, make sure to emphasize the **Caddy Admin API**. 

**Example code-generation prompt:**
> "I am an engineer building a Tauri 2 project. I need a Rust function that takes a `container_name` and `target_port` and uses `reqwest` to send a JSON payload to Caddy's Admin API at `localhost:2019`. The payload should add a reverse_proxy route for `{container_name}.andcake.dev`. Also, provide the global JSON config needed to tell Caddy to use the Cloudflare DNS challenge for wildcard SSL."
