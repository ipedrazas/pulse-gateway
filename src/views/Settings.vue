<script setup lang="ts">
import { onMounted, ref, watch } from "vue";
import { useSettingsStore } from "../stores/settings";

const settings = useSettingsStore();
const message = ref("");
const dnsMessage = ref("");

// DNS credential fields (never pre-filled — secrets stay in keyring)
const selectedProvider = ref<"none" | "cloudflare" | "porkbun">("none");
const cfToken = ref("");
const pbKey = ref("");
const pbSecret = ref("");

onMounted(async () => {
  await settings.fetchSettings();
  await settings.fetchDnsConfig();
  await settings.fetchCertInfo();
  selectedProvider.value = settings.dnsProvider;
});

watch(
  () => settings.dnsProvider,
  (val) => {
    selectedProvider.value = val;
  }
);

async function handleSaveGeneral() {
  message.value = "";
  try {
    await settings.saveSettings();
    message.value = "Settings saved.";
  } catch (e) {
    message.value = "Error: " + String(e);
  }
}

async function handleSaveDns() {
  dnsMessage.value = "";
  try {
    await settings.saveDnsConfig(
      selectedProvider.value,
      cfToken.value || undefined,
      pbKey.value || undefined,
      pbSecret.value || undefined
    );
    // Clear credential fields after save
    cfToken.value = "";
    pbKey.value = "";
    pbSecret.value = "";
    dnsMessage.value = "DNS configuration saved.";
    // Refresh cert info
    await settings.fetchCertInfo();
  } catch (e) {
    dnsMessage.value = "Error: " + String(e);
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
          <label for="caddy-image">Caddy Docker Image</label>
          <input id="caddy-image" v-model="settings.caddyImage" placeholder="caddy:2" />
          <small>Pre-built image with DNS plugins. Change only if using a custom build.</small>
        </div>

        <div class="actions">
          <button type="submit" :disabled="settings.saving">
            {{ settings.saving ? "Saving..." : "Save" }}
          </button>
          <span v-if="message" class="message">{{ message }}</span>
        </div>
      </form>
    </section>

    <!-- DNS / SSL settings -->
    <section class="section">
      <h3>SSL / DNS Provider</h3>
      <form class="settings-form" @submit.prevent="handleSaveDns">
        <div class="field">
          <label for="dns-provider">DNS Provider</label>
          <select id="dns-provider" v-model="selectedProvider">
            <option value="none">None (no SSL)</option>
            <option value="cloudflare">Cloudflare</option>
            <option value="porkbun">Porkbun</option>
          </select>
          <small>Used for DNS-01 challenge to provision wildcard SSL certificates.</small>
        </div>

        <!-- Cloudflare fields -->
        <div v-if="selectedProvider === 'cloudflare'" class="field">
          <label for="cf-token">API Token</label>
          <input
            id="cf-token"
            v-model="cfToken"
            type="password"
            :placeholder="settings.hasCredentials ? '(stored in keyring — leave blank to keep)' : 'Cloudflare API token'"
          />
          <small>Stored securely in the system keyring.</small>
        </div>

        <!-- Porkbun fields -->
        <template v-if="selectedProvider === 'porkbun'">
          <div class="field">
            <label for="pb-key">API Key</label>
            <input
              id="pb-key"
              v-model="pbKey"
              type="password"
              :placeholder="settings.hasCredentials ? '(stored in keyring — leave blank to keep)' : 'Porkbun API key'"
            />
          </div>
          <div class="field">
            <label for="pb-secret">API Secret</label>
            <input
              id="pb-secret"
              v-model="pbSecret"
              type="password"
              :placeholder="settings.hasCredentials ? '(stored in keyring — leave blank to keep)' : 'Porkbun API secret'"
            />
          </div>
          <small class="field-note">Stored securely in the system keyring.</small>
        </template>

        <div class="actions">
          <button type="submit" :disabled="settings.savingDns">
            {{ settings.savingDns ? "Saving..." : "Save DNS Config" }}
          </button>
          <span v-if="dnsMessage" class="message">{{ dnsMessage }}</span>
        </div>
      </form>
    </section>

    <!-- Certificate info -->
    <section class="section">
      <h3>Certificate Status</h3>
      <div class="cert-card">
        <div class="cert-row">
          <span>TLS:</span>
          <span :class="['badge', settings.certInfo.configured ? 'badge-ok' : 'badge-off']">
            {{ settings.certInfo.configured ? "Configured" : "Not configured" }}
          </span>
        </div>
        <div v-if="settings.certInfo.domain" class="cert-row">
          <span>Wildcard:</span>
          <span>{{ settings.certInfo.domain }}</span>
        </div>
        <div v-if="settings.certInfo.expiry" class="cert-row">
          <span>Expires:</span>
          <span>{{ settings.certInfo.expiry }}</span>
        </div>
        <div v-if="settings.certInfo.error" class="cert-error">
          {{ settings.certInfo.error }}
        </div>
        <button class="btn-secondary" @click="handleRefreshCert">Refresh</button>
      </div>
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

.field small,
.field-note {
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

.btn-secondary {
  background: #6c757d;
  margin-top: 0.5rem;
}

.btn-secondary:hover {
  background: #5a6268;
}

select {
  border-radius: 6px;
  border: 1px solid #ccc;
  padding: 0.5rem 0.8rem;
  font-size: 0.95rem;
  font-family: inherit;
  background: #fff;
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

  select {
    background: #1a1a1a;
    border-color: #444;
    color: #f6f6f6;
  }
}
</style>
