<script setup lang="ts">
import { ref, computed, onMounted } from "vue";
import { useGatewayStore } from "../stores/gateway";
import { useSettingsStore } from "../stores/settings";

const gateway = useGatewayStore();
const settings = useSettingsStore();

const newSubdomain = ref("");
const newTargetHost = ref("");
const newPort = ref<number | undefined>();
const error = ref("");

const autoGateways = computed(() =>
  gateway.allGateways.filter((g) => g.source === "auto")
);

const staticGateways = computed(() =>
  gateway.allGateways.filter((g) => g.source === "static")
);

onMounted(async () => {
  await settings.fetchSettings();
  await gateway.init();
});

async function handleStartCaddy() {
  await gateway.startCaddy();
}

async function handleAddRoute() {
  error.value = "";
  if (!newSubdomain.value || !newTargetHost.value || !newPort.value) {
    error.value = "All fields are required";
    return;
  }
  try {
    await gateway.addRoute(newSubdomain.value, newTargetHost.value, newPort.value);
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

function fqdn(subdomain: string): string {
  return settings.domain ? `${subdomain}.${settings.domain}` : subdomain;
}
</script>

<template>
  <div class="dashboard">
    <section class="status-section">
      <h2>Caddy Status</h2>
      <div class="status-card">
        <div class="status-row">
          <span>Container:</span>
          <span :class="['badge', gateway.caddyStatus.running ? 'badge-ok' : 'badge-err']">
            {{ gateway.caddyStatus.running ? "Running" : "Stopped" }}
          </span>
        </div>
        <div class="status-row">
          <span>Admin API:</span>
          <span :class="['badge', gateway.caddyStatus.api_reachable ? 'badge-ok' : 'badge-err']">
            {{ gateway.caddyStatus.api_reachable ? "Reachable" : "Unreachable" }}
          </span>
        </div>
        <div v-if="gateway.caddyStatus.error" class="status-error">
          {{ gateway.caddyStatus.error }}
        </div>
        <button
          v-if="!gateway.caddyStatus.running"
          @click="handleStartCaddy"
          :disabled="gateway.loading"
        >
          {{ gateway.loading ? "Starting..." : "Start Caddy" }}
        </button>
      </div>
    </section>

    <!-- Auto-discovered gateways -->
    <section class="routes-section">
      <h2>Auto-Discovered Gateways</h2>
      <div v-if="!settings.domain" class="domain-warning">
        No domain configured. <router-link to="/settings">Set one in Settings</router-link>.
      </div>
      <table v-if="autoGateways.length > 0" class="routes-table">
        <thead>
          <tr>
            <th>Subdomain</th>
            <th>Container</th>
            <th>Port</th>
            <th>Source</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="gw in autoGateways" :key="gw.subdomain">
            <td>{{ fqdn(gw.subdomain) }}</td>
            <td>{{ gw.container_name || gw.target_host }}</td>
            <td>{{ gw.port }}</td>
            <td><span class="badge badge-auto">Auto</span></td>
          </tr>
        </tbody>
      </table>
      <p v-else class="empty-state">
        No containers detected. Start a Docker container to see it here.
      </p>
    </section>

    <!-- Static routes -->
    <section class="routes-section">
      <h2>Static Routes</h2>
      <form class="add-route-form" @submit.prevent="handleAddRoute">
        <input v-model="newSubdomain" placeholder="subdomain" />
        <input v-model="newTargetHost" placeholder="target host (e.g. localhost)" />
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
              <button class="btn-remove" @click="handleRemoveRoute(route.subdomain)">
                Remove
              </button>
            </td>
          </tr>
        </tbody>
      </table>
      <p v-else class="empty-state">No static routes configured.</p>
    </section>
  </div>
</template>

<style scoped>
.dashboard {
  padding: 1rem;
}

.status-section,
.routes-section {
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

@media (prefers-color-scheme: dark) {
  .status-card {
    background: #1a1a1a;
  }

  .badge-ok {
    background: #1e3a2f;
    color: #75d99a;
  }

  .badge-err {
    background: #3a1e1e;
    color: #e88;
  }

  .badge-auto {
    background: #1e2a3a;
    color: #7ab8e8;
  }

  .domain-warning {
    background: #3a3420;
    color: #e0c870;
  }

  .routes-table th,
  .routes-table td {
    border-bottom-color: #333;
  }
}
</style>
