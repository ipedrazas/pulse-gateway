<script setup lang="ts">
import { ref, computed, onMounted, nextTick, watch } from "vue";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useGatewayStore } from "../stores/gateway";
import { useSettingsStore } from "../stores/settings";

const gateway = useGatewayStore();
const settings = useSettingsStore();

const newSubdomain = ref("");
const newTargetHost = ref("");
const newPort = ref<number | undefined>();
const error = ref("");
const logContainer = ref<HTMLElement | null>(null);

const staticGateways = computed(() =>
  gateway.allGateways.filter((g) => g.source === "static")
);

const hasTls = computed(() => settings.envVars.length > 0);
const certReady = computed(() => !!settings.certInfo.issuer);

onMounted(async () => {
  await settings.fetchSettings();
  await settings.fetchEnvVars();
  await gateway.init();
  await settings.fetchCertInfo();
});

// Re-check cert status when gateways change (routes may arrive after reconciliation)
watch(
  () => gateway.allGateways.length,
  async (newLen, oldLen) => {
    if (newLen > 0 && newLen !== oldLen) {
      await settings.fetchCertInfo();
    }
  }
);

// Auto-scroll log to bottom
watch(
  () => gateway.eventLog.length,
  async () => {
    await nextTick();
    if (logContainer.value) {
      logContainer.value.scrollTop = logContainer.value.scrollHeight;
    }
  }
);

function gatewayStatus(): "ssl" | "proxy" {
  if (hasTls.value && certReady.value) return "ssl";
  return "proxy";
}

function statusLabel(status: string): string {
  switch (status) {
    case "ssl":
      return "SSL";
    case "proxy":
      return "Proxy";
    default:
      return status;
  }
}

function fqdn(subdomain: string): string {
  return settings.domain ? `${subdomain}.${settings.domain}` : subdomain;
}

function gatewayUrl(subdomain: string): string {
  const proto = hasTls.value ? "https" : "http";
  return `${proto}://${fqdn(subdomain)}`;
}

async function openGateway(subdomain: string) {
  try {
    await openUrl(gatewayUrl(subdomain));
  } catch (e) {
    console.error("Failed to open URL:", e);
  }
}

async function handleStartCaddy() {
  await gateway.startCaddy();
  await settings.fetchCertInfo();
}

async function handleStopCaddy() {
  await gateway.stopCaddy();
}

async function handleAddRoute() {
  error.value = "";
  if (!newSubdomain.value || !newTargetHost.value || !newPort.value) {
    error.value = "All fields are required";
    return;
  }
  try {
    await gateway.addRoute(
      newSubdomain.value,
      newTargetHost.value,
      newPort.value
    );
    newSubdomain.value = "";
    newTargetHost.value = "";
    newPort.value = undefined;
  } catch (e) {
    error.value = String(e);
  }
}

async function handleRemoveRoute(subdomain: string) {
  try {
    await gateway.removeRoute(subdomain);
  } catch (e) {
    error.value = String(e);
  }
}
</script>

<template>
  <div class="dashboard">
    <!-- Caddy Status -->
    <section class="section">
      <h2>Caddy Status</h2>
      <div class="status-card">
        <div class="status-row">
          <span>Container:</span>
          <span
            :class="[
              'badge',
              gateway.caddyStatus.running ? 'badge-ok' : 'badge-err',
            ]"
          >
            {{ gateway.caddyStatus.running ? "Running" : "Stopped" }}
          </span>
        </div>
        <div class="status-row">
          <span>Admin API:</span>
          <span
            :class="[
              'badge',
              gateway.caddyStatus.api_reachable ? 'badge-ok' : 'badge-err',
            ]"
          >
            {{
              gateway.caddyStatus.api_reachable ? "Reachable" : "Unreachable"
            }}
          </span>
        </div>
        <div v-if="gateway.caddyStatus.error" class="status-error">
          {{ gateway.caddyStatus.error }}
        </div>
        <div class="status-actions">
          <button
            v-if="!gateway.caddyStatus.running"
            @click="handleStartCaddy"
            :disabled="gateway.loading"
          >
            {{ gateway.loading ? "Starting..." : "Start Caddy" }}
          </button>
          <button
            v-if="gateway.caddyStatus.running"
            class="btn-stop"
            @click="handleStopCaddy"
            :disabled="gateway.loading"
          >
            {{ gateway.loading ? "Stopping..." : "Stop Caddy" }}
          </button>
        </div>
      </div>
    </section>

    <!-- Active Gateways -->
    <section class="section">
      <h2>Active Gateways</h2>
      <div v-if="!settings.domain" class="domain-warning">
        No domain configured.
        <router-link to="/settings">Set one in Settings</router-link>.
      </div>

      <table
        v-if="gateway.allGateways.length > 0"
        class="routes-table"
      >
        <thead>
          <tr>
            <th>Subdomain</th>
            <th>Target</th>
            <th>Port</th>
            <th>Source</th>
            <th>Status</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="gw in gateway.allGateways" :key="gw.subdomain">
            <td>
              <a
                class="subdomain-link"
                href="#"
                @click.prevent="openGateway(gw.subdomain)"
                :title="gatewayUrl(gw.subdomain)"
              >
                {{ fqdn(gw.subdomain) }}
              </a>
            </td>
            <td>{{ gw.container_name || gw.target_host }}</td>
            <td>{{ gw.port }}</td>
            <td>
              <span
                :class="[
                  'badge',
                  gw.source === 'auto' ? 'badge-auto' : 'badge-static',
                ]"
              >
                {{ gw.source === "auto" ? "Auto" : "Static" }}
              </span>
            </td>
            <td>
              <span
                :class="[
                  'badge',
                  `badge-${gatewayStatus()}`,
                ]"
              >
                {{ statusLabel(gatewayStatus()) }}
              </span>
            </td>
          </tr>
        </tbody>
      </table>
      <p v-else class="empty-state">
        No active gateways. Start a Docker container or add a static route.
      </p>
    </section>

    <!-- Static Routes -->
    <section class="section">
      <h2>Static Routes</h2>
      <p class="section-desc">
        Route any service — Docker containers or apps running on your Mac.
        Use <code>localhost</code> for host apps (automatically translated for Docker networking).
      </p>
      <form class="add-route-form" @submit.prevent="handleAddRoute">
        <input v-model="newSubdomain" placeholder="subdomain" />
        <input
          v-model="newTargetHost"
          placeholder="localhost or container name"
        />
        <input v-model.number="newPort" type="number" placeholder="port" />
        <button type="submit">Add Route</button>
      </form>
      <div v-if="error" class="form-error">{{ error }}</div>

      <table v-if="staticGateways.length > 0" class="routes-table">
        <thead>
          <tr>
            <th>Subdomain</th>
            <th>Target</th>
            <th>Port</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="route in staticGateways" :key="route.subdomain">
            <td>{{ fqdn(route.subdomain) }}</td>
            <td>{{ route.target_host }}</td>
            <td>{{ route.port }}</td>
            <td>
              <button
                class="btn-remove"
                @click="handleRemoveRoute(route.subdomain)"
              >
                Remove
              </button>
            </td>
          </tr>
        </tbody>
      </table>
      <p v-else class="empty-state">No static routes configured.</p>
    </section>

    <!-- Event Log -->
    <section class="section">
      <h2>Event Log</h2>
      <div ref="logContainer" class="log-container">
        <div
          v-for="(entry, i) in gateway.eventLog"
          :key="i"
          :class="['log-entry', `log-${entry.level}`]"
        >
          <span class="log-time">{{ entry.timestamp }}</span>
          <span class="log-msg">{{ entry.message }}</span>
        </div>
        <div v-if="gateway.eventLog.length === 0" class="empty-state">
          No events yet.
        </div>
      </div>
    </section>
  </div>
</template>

<style scoped>
.dashboard {
  padding: 1rem;
}

.section {
  margin-bottom: 2rem;
}

.status-card {
  background: var(--card-bg, #fff);
  border-radius: 8px;
  padding: 1rem;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
}

.status-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 0.4rem 0;
}

.badge {
  padding: 0.2rem 0.6rem;
  border-radius: 4px;
  font-size: 0.85rem;
  font-weight: 600;
  display: inline-block;
}

.badge-ok {
  background: #d4edda;
  color: #155724;
}
.badge-err {
  background: #f8d7da;
  color: #721c24;
}
.badge-auto {
  background: #cce5ff;
  color: #004085;
}
.badge-static {
  background: #e2e3e5;
  color: #383d41;
}
.badge-ssl {
  background: #d4edda;
  color: #155724;
}
.badge-proxy {
  background: #fff3cd;
  color: #856404;
}
.badge-error {
  background: #f8d7da;
  color: #721c24;
}

.status-actions {
  margin-top: 0.75rem;
  display: flex;
  gap: 0.5rem;
}

.btn-stop {
  background: #dc3545;
  color: #fff;
}
.btn-stop:hover:not(:disabled) {
  background: #c82333;
}

.status-error {
  color: #721c24;
  margin: 0.5rem 0;
  font-size: 0.9rem;
}

.domain-warning {
  margin-bottom: 1rem;
  padding: 0.5rem;
  background: #fff3cd;
  border-radius: 4px;
  color: #856404;
}

.subdomain-link {
  color: #396cd8;
  text-decoration: none;
  font-weight: 500;
  cursor: pointer;
}
.subdomain-link:hover {
  text-decoration: underline;
}

.section-desc {
  color: #888;
  font-size: 0.9rem;
  margin-bottom: 0.75rem;
}

.section-desc code {
  background: rgba(150, 150, 150, 0.15);
  padding: 0.1rem 0.35rem;
  border-radius: 3px;
  font-size: 0.85rem;
}

.add-route-form {
  display: flex;
  gap: 0.5rem;
  margin-bottom: 1rem;
}
.add-route-form input {
  flex: 1;
}

.form-error {
  color: #721c24;
  margin-bottom: 1rem;
  font-size: 0.9rem;
}

.routes-table {
  width: 100%;
  border-collapse: collapse;
}
.routes-table th,
.routes-table td {
  text-align: left;
  padding: 0.5rem;
  border-bottom: 1px solid #eee;
}

.btn-remove {
  background: #dc3545;
  color: #fff;
  border: none;
  padding: 0.3rem 0.6rem;
  border-radius: 4px;
  cursor: pointer;
  font-size: 0.85rem;
}
.btn-remove:hover {
  background: #c82333;
}

.empty-state {
  color: #888;
  text-align: center;
  padding: 2rem;
}

/* Event Log */
.log-container {
  background: #1a1a1a;
  border-radius: 8px;
  padding: 0.75rem;
  max-height: 300px;
  overflow-y: auto;
  font-family: "SF Mono", "Fira Code", "Cascadia Code", monospace;
  font-size: 0.8rem;
  line-height: 1.5;
}

.log-entry {
  padding: 0.15rem 0;
}

.log-time {
  color: #888;
  margin-right: 0.75rem;
}

.log-info .log-msg {
  color: #b8d4e3;
}
.log-warn .log-msg {
  color: #e0c870;
}
.log-error .log-msg {
  color: #e88;
}

@media (prefers-color-scheme: dark) {
  .status-card {
    background: #1a1a1a;
  }
  .badge-ok,
  .badge-ssl {
    background: #1e3a2f;
    color: #75d99a;
  }
  .badge-err,
  .badge-error {
    background: #3a1e1e;
    color: #e88;
  }
  .badge-auto {
    background: #1e2a3a;
    color: #7ab8e8;
  }
  .badge-static {
    background: #2a2a2a;
    color: #aaa;
  }
  .badge-proxy {
    background: #3a3420;
    color: #e0c870;
  }
  .domain-warning {
    background: #3a3420;
    color: #e0c870;
  }
  .routes-table th,
  .routes-table td {
    border-bottom-color: #333;
  }
  .subdomain-link {
    color: #7ab8e8;
  }
}

@media (prefers-color-scheme: light) {
  .log-container {
    background: #f0f0f0;
  }
  .log-time {
    color: #666;
  }
  .log-info .log-msg {
    color: #2a5a7a;
  }
  .log-warn .log-msg {
    color: #8a6d00;
  }
  .log-error .log-msg {
    color: #a03030;
  }
}
</style>
