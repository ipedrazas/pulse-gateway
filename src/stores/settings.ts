import { defineStore } from "pinia";
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

export interface AppConfig {
  domain: string;
  caddy_image: string;
  dns_provider: "none" | "cloudflare" | "porkbun";
}

export interface DnsConfig {
  provider: "none" | "cloudflare" | "porkbun";
  has_credentials: boolean;
}

export interface CertInfo {
  configured: boolean;
  domain: string | null;
  expiry: string | null;
  error: string | null;
}

export const useSettingsStore = defineStore("settings", () => {
  const domain = ref("");
  const caddyImage = ref("caddy:2");
  const saving = ref(false);

  // DNS config
  const dnsProvider = ref<"none" | "cloudflare" | "porkbun">("none");
  const hasCredentials = ref(false);
  const savingDns = ref(false);

  // Cert info
  const certInfo = ref<CertInfo>({
    configured: false,
    domain: null,
    expiry: null,
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
      });
      domain.value = config.domain;
      caddyImage.value = config.caddy_image;
    } finally {
      saving.value = false;
    }
  }

  async function fetchDnsConfig() {
    try {
      const config: DnsConfig = await invoke("get_dns_config");
      dnsProvider.value = config.provider;
      hasCredentials.value = config.has_credentials;
    } catch (e) {
      console.error("Failed to load DNS config:", e);
    }
  }

  async function saveDnsConfig(
    provider: "none" | "cloudflare" | "porkbun",
    apiToken?: string,
    apiKey?: string,
    apiSecret?: string
  ) {
    savingDns.value = true;
    try {
      const config: DnsConfig = await invoke("save_dns_config", {
        provider,
        apiToken: apiToken || null,
        apiKey: apiKey || null,
        apiSecret: apiSecret || null,
      });
      dnsProvider.value = config.provider;
      hasCredentials.value = config.has_credentials;
    } finally {
      savingDns.value = false;
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
    saving,
    dnsProvider,
    hasCredentials,
    savingDns,
    certInfo,
    fetchSettings,
    saveSettings,
    fetchDnsConfig,
    saveDnsConfig,
    fetchCertInfo,
  };
});
