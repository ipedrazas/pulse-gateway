import { defineStore } from "pinia";
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

export interface AppConfig {
  domain: string;
  caddy_image: string;
}

export const useSettingsStore = defineStore("settings", () => {
  const domain = ref("");
  const caddyImage = ref("caddy:2");
  const saving = ref(false);

  async function fetchSettings() {
    try {
      const config: AppConfig = await invoke("get_settings");
      domain.value = config.domain;
      caddyImage.value = config.caddy_image;
    } catch (e) {
      console.error("Failed to load settings:", e);
    }
  }

  async function saveSettings() {
    saving.value = true;
    try {
      const config: AppConfig = await invoke("save_settings", {
        domain: domain.value,
        caddyImage: caddyImage.value,
      });
      domain.value = config.domain;
      caddyImage.value = config.caddy_image;
    } catch (e) {
      throw e;
    } finally {
      saving.value = false;
    }
  }

  return { domain, caddyImage, saving, fetchSettings, saveSettings };
});
