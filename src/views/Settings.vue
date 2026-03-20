<script setup lang="ts">
import { onMounted, ref } from "vue";
import { getVersion } from "@tauri-apps/api/app";
import { useSettingsStore } from "../stores/settings";

const settings = useSettingsStore();
const message = ref("");
const envMessage = ref("");

// New env var form
const appVersion = ref("");
const newEnvKey = ref("");
const newEnvValue = ref("");

onMounted(async () => {
  appVersion.value = await getVersion();
  await settings.fetchSettings();
  await settings.fetchEnvVars();
  await settings.fetchCertInfo();
});

async function handleSaveGeneral() {
  message.value = "";
  try {
    await settings.saveSettings();
    message.value = "Settings saved.";
  } catch (e) {
    message.value = "Error: " + String(e);
  }
}

async function handleAddEnvVar() {
  envMessage.value = "";
  if (!newEnvKey.value || !newEnvValue.value) {
    envMessage.value = "Both key and value are required.";
    return;
  }
  try {
    await settings.saveEnvVar(newEnvKey.value, newEnvValue.value);
    newEnvKey.value = "";
    newEnvValue.value = "";
    envMessage.value = "Env var saved. Restart Caddy to apply.";
  } catch (e) {
    envMessage.value = "Error: " + String(e);
  }
}

async function handleRemoveEnvVar(key: string) {
  try {
    await settings.removeEnvVar(key);
    envMessage.value = "Removed. Restart Caddy to apply.";
  } catch (e) {
    envMessage.value = "Error: " + String(e);
  }
}

async function handleRefreshCert() {
  await settings.fetchCertInfo();
}
</script>

<template>
  <div class="settings">
    <h2>Settings</h2>

    <!-- General settings -->
    <section class="section">
      <h3>General</h3>
      <form class="settings-form" @submit.prevent="handleSaveGeneral">
        <div class="field">
          <label for="domain">Root Domain</label>
          <input id="domain" v-model="settings.domain" placeholder="e.g. myapp.dev" />
          <small>Subdomains will be created as *.{{ settings.domain || "yourdomain.dev" }}</small>
        </div>

        <div class="field">
          <label for="dns-provider">DNS Provider</label>
          <select id="dns-provider" v-model="settings.dnsProvider">
            <option value="cloudflare">Cloudflare</option>
            <option value="porkbun">Porkbun</option>
          </select>
          <small>DNS provider used for wildcard certificate DNS-01 challenge.</small>
        </div>

        <div class="field">
          <label for="caddy-image">Caddy Docker Image</label>
          <input id="caddy-image" v-model="settings.caddyImage" placeholder="caddy:2" />
          <small v-if="settings.dnsProvider === 'cloudflare'">
            Use an image with the Cloudflare DNS plugin (e.g.
            ghcr.io/caddybuilds/caddy-cloudflare:latest).
          </small>
          <small v-else-if="settings.dnsProvider === 'porkbun'">
            Use an image with the Porkbun DNS plugin (e.g. ghcr.io/caddy-dns/porkbun:latest).
          </small>
        </div>

        <div class="actions">
          <button type="submit" :disabled="settings.saving">
            {{ settings.saving ? "Saving..." : "Save" }}
          </button>
          <span v-if="message" class="message">{{ message }}</span>
        </div>
      </form>
    </section>

    <!-- Caddy Environment Variables -->
    <section class="section">
      <h3>Caddy Environment Variables</h3>
      <p class="section-desc">
        Set environment variables passed to the Caddy container. Values are stored securely in the
        system keyring. Restart Caddy after changes.
      </p>

      <form class="env-form" @submit.prevent="handleAddEnvVar">
        <input v-model="newEnvKey" placeholder="VARIABLE_NAME" class="env-key" />
        <input v-model="newEnvValue" type="password" placeholder="value" class="env-value" />
        <button type="submit" :disabled="settings.savingEnv">Add</button>
      </form>
      <div v-if="envMessage" class="env-message">{{ envMessage }}</div>

      <table v-if="settings.envVars.length > 0" class="env-table">
        <thead>
          <tr>
            <th>Variable</th>
            <th>Status</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="[key, hasValue] in settings.envVars" :key="key">
            <td class="env-key-cell">{{ key }}</td>
            <td>
              <span :class="['badge', hasValue ? 'badge-ok' : 'badge-off']">
                {{ hasValue ? "Stored" : "Missing" }}
              </span>
            </td>
            <td>
              <button class="btn-remove" @click="handleRemoveEnvVar(key)">Remove</button>
            </td>
          </tr>
        </tbody>
      </table>
      <p v-else class="empty-state">No environment variables configured.</p>
    </section>

    <!-- Certificate info -->
    <section class="section">
      <h3>Certificate Status</h3>
      <div class="cert-card">
        <div class="cert-row">
          <span>Env vars:</span>
          <span :class="['badge', settings.certInfo.has_env_vars ? 'badge-ok' : 'badge-off']">
            {{ settings.certInfo.has_env_vars ? "Configured" : "None" }}
          </span>
        </div>
        <div v-if="settings.certInfo.domain" class="cert-row">
          <span>Wildcard:</span>
          <span>{{ settings.certInfo.domain }}</span>
        </div>
        <div v-if="settings.certInfo.subject_alt_names" class="cert-row">
          <span>SANs:</span>
          <span>{{ settings.certInfo.subject_alt_names }}</span>
        </div>
        <div v-if="settings.certInfo.issuer" class="cert-row">
          <span>Issuer:</span>
          <span>{{ settings.certInfo.issuer }}</span>
        </div>
        <div v-if="settings.certInfo.not_before" class="cert-row">
          <span>Issued:</span>
          <span>{{ settings.certInfo.not_before }}</span>
        </div>
        <div v-if="settings.certInfo.not_after" class="cert-row">
          <span>Expires:</span>
          <span>{{ settings.certInfo.not_after }}</span>
        </div>
        <div v-if="settings.certInfo.error" class="cert-error">
          {{ settings.certInfo.error }}
        </div>
        <p class="cert-note">
          Caddy auto-renews certificates ~30 days before expiry. Restart Caddy to force a renewal
          check.
        </p>
        <button class="btn-secondary" @click="handleRefreshCert">Refresh</button>
      </div>
    </section>

    <!-- About -->
    <section class="section about">
      <p>Pulse Gateway v{{ appVersion }}</p>
    </section>
  </div>
</template>

<style scoped>
.settings {
  padding: 1rem;
}

.section {
  margin-bottom: 2rem;
}

.section h3 {
  margin-bottom: 0.75rem;
  border-bottom: 1px solid #eee;
  padding-bottom: 0.4rem;
}

.section-desc {
  color: #888;
  font-size: 0.9rem;
  margin-bottom: 1rem;
}

.settings-form {
  max-width: 500px;
}

.field {
  margin-bottom: 1.5rem;
}

.field label {
  display: block;
  font-weight: 600;
  margin-bottom: 0.3rem;
}

.field input,
.field select {
  width: 100%;
  box-sizing: border-box;
}

.field small {
  display: block;
  color: #888;
  margin-top: 0.3rem;
  font-size: 0.85rem;
}

.actions {
  display: flex;
  align-items: center;
  gap: 1rem;
}

.message {
  font-size: 0.9rem;
  color: #555;
}

/* Env var editor */
.env-form {
  display: flex;
  gap: 0.5rem;
  margin-bottom: 0.75rem;
  max-width: 600px;
}

.env-key {
  flex: 1;
  font-family: "SF Mono", "Fira Code", monospace;
  font-size: 0.9rem;
}

.env-value {
  flex: 1.5;
}

.env-message {
  font-size: 0.9rem;
  color: #555;
  margin-bottom: 0.75rem;
}

.env-table {
  width: 100%;
  max-width: 600px;
  border-collapse: collapse;
}

.env-table th,
.env-table td {
  text-align: left;
  padding: 0.5rem;
  border-bottom: 1px solid #eee;
}

.env-key-cell {
  font-family: "SF Mono", "Fira Code", monospace;
  font-size: 0.9rem;
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

.badge-off {
  background: #e2e3e5;
  color: #383d41;
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
  padding: 1rem 0;
}

.cert-card {
  background: var(--card-bg, #fff);
  border-radius: 8px;
  padding: 1rem;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
  max-width: 500px;
}

.cert-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 0.4rem 0;
}

.cert-error {
  color: #721c24;
  font-size: 0.9rem;
  margin: 0.5rem 0;
}

.cert-note {
  color: #888;
  font-size: 0.85rem;
  margin: 0.75rem 0 0.5rem;
}

.about {
  text-align: center;
  color: #999;
  font-size: 0.85rem;
}

.btn-secondary {
  background: #6c757d;
  margin-top: 0.5rem;
}

.btn-secondary:hover {
  background: #5a6268;
}

@media (prefers-color-scheme: dark) {
  .section h3 {
    border-bottom-color: #333;
  }

  .cert-card {
    background: #1a1a1a;
  }

  .badge-ok {
    background: #1e3a2f;
    color: #75d99a;
  }

  .badge-off {
    background: #2a2a2a;
    color: #aaa;
  }

  .env-table th,
  .env-table td {
    border-bottom-color: #333;
  }
}
</style>
