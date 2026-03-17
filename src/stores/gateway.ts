import { defineStore } from "pinia";
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export interface Gateway {
  subdomain: string;
  target_host: string;
  port: number;
  source: "static" | "auto";
  container_id?: string;
  container_name?: string;
}

export interface CaddyStatus {
  running: boolean;
  api_reachable: boolean;
  error: string | null;
}

export const useGatewayStore = defineStore("gateway", () => {
  const allGateways = ref<Gateway[]>([]);
  const staticRoutes = ref<Gateway[]>([]);
  const caddyStatus = ref<CaddyStatus>({
    running: false,
    api_reachable: false,
    error: null,
  });
  const loading = ref(false);
  let eventUnlisten: (() => void) | null = null;

  async function init() {
    await setupEventListener();
    await fetchStatus();
    await fetchAllGateways();
    await fetchStaticRoutes();
  }

  async function setupEventListener() {
    if (eventUnlisten) return;
    eventUnlisten = await listen<Gateway[]>("gateways-changed", (event) => {
      allGateways.value = event.payload;
    });
  }

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
      await fetchAllGateways();
      await fetchStaticRoutes();
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

  async function fetchAllGateways() {
    try {
      allGateways.value = await invoke("get_all_gateways");
    } catch (e) {
      console.error("Failed to fetch gateways:", e);
    }
  }

  async function fetchStaticRoutes() {
    try {
      staticRoutes.value = await invoke("get_routes");
    } catch (e) {
      console.error("Failed to fetch static routes:", e);
    }
  }

  async function addRoute(subdomain: string, targetHost: string, port: number) {
    staticRoutes.value = await invoke("add_route", {
      subdomain,
      targetHost,
      port,
    });
  }

  async function removeRoute(subdomain: string) {
    staticRoutes.value = await invoke("remove_route", { subdomain });
  }

  return {
    allGateways,
    staticRoutes,
    caddyStatus,
    loading,
    init,
    fetchStatus,
    startCaddy,
    fetchAllGateways,
    fetchStaticRoutes,
    addRoute,
    removeRoute,
  };
});
