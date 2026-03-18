import { defineStore } from "pinia";
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

export type DnsProvider = "cloudflare" | "porkbun";

export interface AppConfig {
  domain: string;
  caddy_image: string;
  dns_provider: DnsProvider;
}

export interface CertInfo {
  has_env_vars: boolean;
  domain: string | null;
  issuer: string | null;
  not_before: string | null;
  not_after: string | null;
  subject_alt_names: string | null;
  error: string | null;
}

/** [key, hasStoredValue] */
export type EnvVarInfo = [string, boolean];

export const useSettingsStore = defineStore("settings", () => {
  const domain = ref("");
  const caddyImage = ref("caddy:2");
  const dnsProvider = ref<DnsProvider>("cloudflare");
  const saving = ref(false);

  // Env vars for Caddy container
  const envVars = ref<EnvVarInfo[]>([]);
  const savingEnv = ref(false);

  // Cert info
  const certInfo = ref<CertInfo>({
    has_env_vars: false,
    domain: null,
    issuer: null,
    not_before: null,
    not_after: null,
    subject_alt_names: null,
    error: null,
  });

  async function fetchSettings() {
    try {
      const config: AppConfig = await invoke("get_settings");
      domain.value = config.domain;
      caddyImage.value = config.caddy_image;
      dnsProvider.value = config.dns_provider;
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
        dnsProvider: dnsProvider.value,
      });
      domain.value = config.domain;
      caddyImage.value = config.caddy_image;
      dnsProvider.value = config.dns_provider;
    } finally {
      saving.value = false;
    }
  }

  async function fetchEnvVars() {
    try {
      envVars.value = await invoke("get_env_vars");
    } catch (e) {
      console.error("Failed to load env vars:", e);
    }
  }

  async function saveEnvVar(key: string, value: string) {
    savingEnv.value = true;
    try {
      envVars.value = await invoke("save_env_var", { key, value });
    } finally {
      savingEnv.value = false;
    }
  }

  async function removeEnvVar(key: string) {
    try {
      envVars.value = await invoke("remove_env_var", { key });
    } catch (e) {
      console.error("Failed to remove env var:", e);
    }
  }

  async function fetchCertInfo() {
    try {
      certInfo.value = await invoke("get_cert_info");
    } catch (e) {
      console.error("Failed to get cert info:", e);
    }
  }

  return {
    domain,
    caddyImage,
    dnsProvider,
    saving,
    envVars,
    savingEnv,
    certInfo,
    fetchSettings,
    saveSettings,
    fetchEnvVars,
    saveEnvVar,
    removeEnvVar,
    fetchCertInfo,
  };
});
