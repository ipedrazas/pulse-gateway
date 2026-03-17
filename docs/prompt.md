"I'm building a Tauri 2 (Rust/React) app called 'Pulse Gateway'. 

The goal is to manage a Caddy Docker container that acts as a reverse proxy for other local containers.

Technical Specs:
1. Backend (Rust): Use 'bollard' to monitor the Docker socket.
2. Proxy Logic: When a container starts with an exposed port, the app should:
   - Identify the container name and exposed port.
   - Use 'reqwest' to call Caddy's Admin API (port 2019) to add a new route: {container_name}.andcake.dev -> container:port.
3. SSL: Configure Caddy to use the DNS-01 challenge with Cloudflare/Porkbun for a wildcard certificate (*.andcake.dev).
4. UI (React): A dashboard showing:
   - Connection status to Docker and Caddy.
   - List of active proxied routes. Proxied routes will have a name, port, status, permanent flag (this is if I want to have the name/port saved regardless of the running container)
   - A settings page for API Keys (Cloudflare/Porkbun).
   - Proxied routes can be created/modified by the user:
    - create a route `my-app` with port `3001` as `my-app.andcake.dev`
    - create a route `img` with port `3005` with docker image `git.andcake.dev/ivan/appimg:latest` as `img.andcake.dev` (every time I run a container from `git.andcake.dev/ivan/appimg:latest`)

Can you provide the Rust code for a service that listens to Docker events and a helper function that formats the JSON payload for Caddy's /config/ API?"
