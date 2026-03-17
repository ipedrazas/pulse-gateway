import { defineStore } from "pinia";
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

export interface Gateway {
  subdomain: string;
  target_host: string;
  port: number;
}

export interface CaddyStatus {
  running: boolean;
  api_reachable: boolean;
  error: string | null;
}

export const useGatewayStore = defineStore("gateway", () => {
  const routes = ref<Gateway[]>([]);
  const caddyStatus = ref<CaddyStatus>({
    running: false,
    api_reachable: false,
    error: null,
  });
  const loading = ref(false);

  async function fetchStatus() {
    try {
      caddyStatus.value = await invoke("get_caddy_status");
    } catch (e) {
      caddyStatus.value = {
        running: false,
        api_reachable: false,
        error: String(e),
      };
    }
  }

  async function startCaddy() {
    loading.value = true;
    try {
      caddyStatus.value = await invoke("start_caddy");
      await fetchRoutes();
    } catch (e) {
      caddyStatus.value = {
        running: false,
        api_reachable: false,
        error: String(e),
      };
    } finally {
      loading.value = false;
    }
  }

  async function fetchRoutes() {
    try {
      routes.value = await invoke("get_routes");
    } catch (e) {
      console.error("Failed to fetch routes:", e);
    }
  }

  async function addRoute(subdomain: string, targetHost: string, port: number) {
    try {
      routes.value = await invoke("add_route", {
        subdomain,
        targetHost,
        port,
      });
    } catch (e) {
      throw e;
    }
  }

  async function removeRoute(subdomain: string) {
    try {
      routes.value = await invoke("remove_route", { subdomain });
    } catch (e) {
      throw e;
    }
  }

  return {
    routes,
    caddyStatus,
    loading,
    fetchStatus,
    startCaddy,
    fetchRoutes,
    addRoute,
    removeRoute,
  };
});
