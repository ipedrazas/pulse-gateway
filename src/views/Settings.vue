<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useSettingsStore } from "../stores/settings";

const settings = useSettingsStore();
const message = ref("");

onMounted(async () => {
  await settings.fetchSettings();
});

async function handleSave() {
  message.value = "";
  try {
    await settings.saveSettings();
    message.value = "Settings saved.";
  } catch (e) {
    message.value = "Error: " + String(e);
  }
}
</script>

<template>
  <div class="settings">
    <h2>Settings</h2>
    <form class="settings-form" @submit.prevent="handleSave">
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
  </div>
</template>

<style scoped>
.settings {
  padding: 1rem;
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

.field input {
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
</style>
