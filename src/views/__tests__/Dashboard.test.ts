import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createRouter, createWebHistory } from "vue-router";

vi.mock("@tauri-apps/api/core");
vi.mock("@tauri-apps/api/event");
vi.mock("@tauri-apps/plugin-opener");

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import Dashboard from "../Dashboard.vue";

const mockedInvoke = vi.mocked(invoke);
const mockedListen = vi.mocked(listen);

const router = createRouter({
  history: createWebHistory(),
  routes: [
    { path: "/", component: Dashboard },
    { path: "/settings", component: { template: "<div>Settings</div>" } },
  ],
});

function mountDashboard() {
  return mount(Dashboard, {
    global: {
      plugins: [createPinia(), router],
    },
  });
}

describe("Dashboard.vue", () => {
  beforeEach(async () => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
    mockedListen.mockResolvedValue(() => {});

    mockedInvoke.mockImplementation(async (cmd: string) => {
      switch (cmd) {
        case "get_settings":
          return { domain: "test.dev", caddy_image: "caddy:2", dns_provider: "cloudflare" };
        case "get_env_vars":
          return [["CLOUDFLARE_API_TOKEN", true]];
        case "get_cert_info":
          return {
            has_env_vars: true,
            domain: "*.test.dev",
            issuer: "Let's Encrypt",
            not_before: null,
            not_after: null,
            subject_alt_names: "app.test.dev",
            error: null,
          };
        case "get_caddy_status":
          return { running: true, api_reachable: true, error: null };
        case "get_all_gateways":
          return [
            {
              subdomain: "myapp",
              target_host: "container-1",
              port: 80,
              source: "auto",
              container_id: "abc",
              container_name: "container-1",
            },
          ];
        case "get_routes":
          return [];
        case "get_event_log":
          return [];
        default:
          return {};
      }
    });

    router.push("/");
    await router.isReady();
  });

  it("renders Caddy Status section", async () => {
    const wrapper = mountDashboard();
    await flushPromises();
    expect(wrapper.text()).toContain("Caddy Status");
  });

  it("shows Running badge when Caddy is running", async () => {
    const wrapper = mountDashboard();
    await flushPromises();
    expect(wrapper.text()).toContain("Running");
    expect(wrapper.text()).toContain("Reachable");
  });

  it("shows Stop Caddy button when running", async () => {
    const wrapper = mountDashboard();
    await flushPromises();
    expect(wrapper.find(".btn-stop").exists()).toBe(true);
    expect(wrapper.find(".btn-stop").text()).toBe("Stop Caddy");
  });

  it("renders Active Gateways with routes", async () => {
    const wrapper = mountDashboard();
    await flushPromises();
    expect(wrapper.text()).toContain("Active Gateways");
    expect(wrapper.text()).toContain("myapp.test.dev");
  });

  it("shows SSL status when cert is ready", async () => {
    const wrapper = mountDashboard();
    await flushPromises();
    expect(wrapper.text()).toContain("SSL");
  });

  it("renders Static Routes section", async () => {
    const wrapper = mountDashboard();
    await flushPromises();
    expect(wrapper.text()).toContain("Static Routes");
    expect(wrapper.text()).toContain("Add Route");
  });

  it("renders Event Log section", async () => {
    const wrapper = mountDashboard();
    await flushPromises();
    expect(wrapper.text()).toContain("Event Log");
  });

  it("shows Start Caddy when not running", async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      switch (cmd) {
        case "get_settings":
          return { domain: "test.dev", caddy_image: "caddy:2", dns_provider: "cloudflare" };
        case "get_env_vars":
          return [];
        case "get_cert_info":
          return {
            has_env_vars: false,
            domain: null,
            issuer: null,
            not_before: null,
            not_after: null,
            subject_alt_names: null,
            error: null,
          };
        case "get_caddy_status":
          return { running: false, api_reachable: false, error: null };
        case "get_all_gateways":
          return [];
        case "get_routes":
          return [];
        case "get_event_log":
          return [];
        default:
          return {};
      }
    });

    const wrapper = mountDashboard();
    await flushPromises();

    const startBtn = wrapper.findAll("button").find((b) => b.text() === "Start Caddy");
    expect(startBtn).toBeDefined();
  });

  it("shows Proxy status when no TLS configured", async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      switch (cmd) {
        case "get_settings":
          return { domain: "test.dev", caddy_image: "caddy:2", dns_provider: "cloudflare" };
        case "get_env_vars":
          return [];
        case "get_cert_info":
          return {
            has_env_vars: false,
            domain: null,
            issuer: null,
            not_before: null,
            not_after: null,
            subject_alt_names: null,
            error: null,
          };
        case "get_caddy_status":
          return { running: true, api_reachable: true, error: null };
        case "get_all_gateways":
          return [{ subdomain: "app", target_host: "c", port: 80, source: "auto" }];
        case "get_routes":
          return [];
        case "get_event_log":
          return [];
        default:
          return {};
      }
    });

    const wrapper = mountDashboard();
    await flushPromises();
    expect(wrapper.text()).toContain("Proxy");
  });

  it("shows domain warning when no domain configured", async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      switch (cmd) {
        case "get_settings":
          return { domain: "", caddy_image: "caddy:2", dns_provider: "cloudflare" };
        case "get_env_vars":
          return [];
        case "get_cert_info":
          return {
            has_env_vars: false,
            domain: null,
            issuer: null,
            not_before: null,
            not_after: null,
            subject_alt_names: null,
            error: null,
          };
        case "get_caddy_status":
          return { running: false, api_reachable: false, error: null };
        case "get_all_gateways":
          return [];
        case "get_routes":
          return [];
        case "get_event_log":
          return [];
        default:
          return {};
      }
    });

    const wrapper = mountDashboard();
    await flushPromises();
    expect(wrapper.text()).toContain("No domain configured");
  });

  it("shows empty state when no gateways", async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      switch (cmd) {
        case "get_settings":
          return { domain: "test.dev", caddy_image: "caddy:2", dns_provider: "cloudflare" };
        case "get_env_vars":
          return [];
        case "get_cert_info":
          return {
            has_env_vars: false,
            domain: null,
            issuer: null,
            not_before: null,
            not_after: null,
            subject_alt_names: null,
            error: null,
          };
        case "get_caddy_status":
          return { running: true, api_reachable: true, error: null };
        case "get_all_gateways":
          return [];
        case "get_routes":
          return [];
        case "get_event_log":
          return [];
        default:
          return {};
      }
    });

    const wrapper = mountDashboard();
    await flushPromises();
    expect(wrapper.text()).toContain("No active gateways");
  });

  it("shows no static routes message", async () => {
    const wrapper = mountDashboard();
    await flushPromises();
    expect(wrapper.text()).toContain("No static routes configured");
  });

  it("shows error when Caddy has error", async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      switch (cmd) {
        case "get_settings":
          return { domain: "test.dev", caddy_image: "caddy:2", dns_provider: "cloudflare" };
        case "get_env_vars":
          return [];
        case "get_cert_info":
          return {
            has_env_vars: false,
            domain: null,
            issuer: null,
            not_before: null,
            not_after: null,
            subject_alt_names: null,
            error: null,
          };
        case "get_caddy_status":
          return {
            running: false,
            api_reachable: false,
            error: "Docker is not running",
          };
        case "get_all_gateways":
          return [];
        case "get_routes":
          return [];
        case "get_event_log":
          return [];
        default:
          return {};
      }
    });

    const wrapper = mountDashboard();
    await flushPromises();
    expect(wrapper.text()).toContain("Docker is not running");
  });

  it("shows add route form fields", async () => {
    const wrapper = mountDashboard();
    await flushPromises();

    const inputs = wrapper.findAll(".add-route-form input");
    expect(inputs.length).toBe(3);
  });

  it("validates add route requires all fields", async () => {
    const wrapper = mountDashboard();
    await flushPromises();

    // Submit with empty fields
    const form = wrapper.find(".add-route-form");
    await form.trigger("submit");
    await flushPromises();

    expect(wrapper.text()).toContain("All fields are required");
  });

  it("shows multiple gateways with correct source badges", async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      switch (cmd) {
        case "get_settings":
          return { domain: "test.dev", caddy_image: "caddy:2", dns_provider: "cloudflare" };
        case "get_env_vars":
          return [["TOKEN", true]];
        case "get_cert_info":
          return {
            has_env_vars: true,
            domain: "*.test.dev",
            issuer: "LE",
            not_before: null,
            not_after: null,
            subject_alt_names: "a.test.dev",
            error: null,
          };
        case "get_caddy_status":
          return { running: true, api_reachable: true, error: null };
        case "get_all_gateways":
          return [
            { subdomain: "app1", target_host: "c1", port: 80, source: "auto" },
            { subdomain: "app2", target_host: "localhost", port: 3000, source: "static" },
          ];
        case "get_routes":
          return [{ subdomain: "app2", target_host: "localhost", port: 3000, source: "static" }];
        case "get_event_log":
          return [];
        default:
          return {};
      }
    });

    const wrapper = mountDashboard();
    await flushPromises();

    expect(wrapper.text()).toContain("app1.test.dev");
    expect(wrapper.text()).toContain("app2.test.dev");
    expect(wrapper.text()).toContain("Auto");
    expect(wrapper.text()).toContain("Static");
  });

  it("shows event log entries", async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      switch (cmd) {
        case "get_settings":
          return { domain: "test.dev", caddy_image: "caddy:2", dns_provider: "cloudflare" };
        case "get_env_vars":
          return [];
        case "get_cert_info":
          return {
            has_env_vars: false,
            domain: null,
            issuer: null,
            not_before: null,
            not_after: null,
            subject_alt_names: null,
            error: null,
          };
        case "get_caddy_status":
          return { running: true, api_reachable: true, error: null };
        case "get_all_gateways":
          return [];
        case "get_routes":
          return [];
        case "get_event_log":
          return [
            { timestamp: "12:00:00", level: "info", message: "Watcher started" },
            { timestamp: "12:00:01", level: "error", message: "Something failed" },
          ];
        default:
          return {};
      }
    });

    const wrapper = mountDashboard();
    await flushPromises();
    expect(wrapper.text()).toContain("Watcher started");
    expect(wrapper.text()).toContain("Something failed");
  });

  it("shows Stopped badge when Caddy is not running", async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      switch (cmd) {
        case "get_settings":
          return { domain: "test.dev", caddy_image: "caddy:2", dns_provider: "cloudflare" };
        case "get_env_vars":
          return [];
        case "get_cert_info":
          return {
            has_env_vars: false,
            domain: null,
            issuer: null,
            not_before: null,
            not_after: null,
            subject_alt_names: null,
            error: null,
          };
        case "get_caddy_status":
          return { running: false, api_reachable: false, error: null };
        case "get_all_gateways":
          return [];
        case "get_routes":
          return [];
        case "get_event_log":
          return [];
        default:
          return {};
      }
    });

    const wrapper = mountDashboard();
    await flushPromises();
    expect(wrapper.text()).toContain("Stopped");
    expect(wrapper.text()).toContain("Unreachable");
  });

  it("shows Unreachable when API is down but container running", async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      switch (cmd) {
        case "get_settings":
          return { domain: "test.dev", caddy_image: "caddy:2", dns_provider: "cloudflare" };
        case "get_env_vars":
          return [];
        case "get_cert_info":
          return {
            has_env_vars: false,
            domain: null,
            issuer: null,
            not_before: null,
            not_after: null,
            subject_alt_names: null,
            error: null,
          };
        case "get_caddy_status":
          return { running: true, api_reachable: false, error: null };
        case "get_all_gateways":
          return [];
        case "get_routes":
          return [];
        case "get_event_log":
          return [];
        default:
          return {};
      }
    });

    const wrapper = mountDashboard();
    await flushPromises();
    expect(wrapper.text()).toContain("Running");
    expect(wrapper.text()).toContain("Unreachable");
  });

  it("shows static route in the Static Routes table", async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      switch (cmd) {
        case "get_settings":
          return { domain: "test.dev", caddy_image: "caddy:2", dns_provider: "cloudflare" };
        case "get_env_vars":
          return [];
        case "get_cert_info":
          return {
            has_env_vars: false,
            domain: null,
            issuer: null,
            not_before: null,
            not_after: null,
            subject_alt_names: null,
            error: null,
          };
        case "get_caddy_status":
          return { running: true, api_reachable: true, error: null };
        case "get_all_gateways":
          return [{ subdomain: "myapi", target_host: "localhost", port: 4000, source: "static" }];
        case "get_routes":
          return [{ subdomain: "myapi", target_host: "localhost", port: 4000, source: "static" }];
        case "get_event_log":
          return [];
        default:
          return {};
      }
    });

    const wrapper = mountDashboard();
    await flushPromises();

    // Should appear in the Static Routes table with Remove button
    const tables = wrapper.findAll(".routes-table");
    const staticTable = tables[tables.length - 1];
    expect(staticTable.text()).toContain("myapi");
    expect(staticTable.text()).toContain("localhost");
    expect(staticTable.text()).toContain("4000");
    expect(staticTable.text()).toContain("Remove");
  });

  it("shows No events yet when log is empty", async () => {
    const wrapper = mountDashboard();
    await flushPromises();
    expect(wrapper.text()).toContain("No events yet");
  });

  it("shows helper text for static routes", async () => {
    const wrapper = mountDashboard();
    await flushPromises();
    expect(wrapper.text()).toContain("localhost");
    expect(wrapper.text()).toContain("Docker networking");
  });
});
