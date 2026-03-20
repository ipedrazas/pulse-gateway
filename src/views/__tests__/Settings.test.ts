import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";

vi.mock("@tauri-apps/api/core");
vi.mock("@tauri-apps/api/app");

import { invoke } from "@tauri-apps/api/core";
import Settings from "../Settings.vue";

const mockedInvoke = vi.mocked(invoke);

function mountSettings() {
  return mount(Settings, {
    global: {
      plugins: [createPinia()],
    },
  });
}

describe("Settings.vue", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();

    // Default mock responses for the three calls in onMounted
    mockedInvoke.mockImplementation(async (cmd: string) => {
      switch (cmd) {
        case "get_settings":
          return {
            domain: "test.dev",
            caddy_image: "caddy:2",
            dns_provider: "cloudflare",
          };
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
        default:
          return {};
      }
    });
  });

  it("renders the Settings heading", async () => {
    const wrapper = mountSettings();
    await flushPromises();
    expect(wrapper.find("h2").text()).toBe("Settings");
  });

  it("displays the domain from settings", async () => {
    const wrapper = mountSettings();
    await flushPromises();
    const input = wrapper.find("#domain");
    expect((input.element as HTMLInputElement).value).toBe("test.dev");
  });

  it("displays DNS provider selector", async () => {
    const wrapper = mountSettings();
    await flushPromises();
    const select = wrapper.find("#dns-provider");
    expect(select.exists()).toBe(true);
    expect((select.element as HTMLSelectElement).value).toBe("cloudflare");
  });

  it("displays env var with stored status", async () => {
    const wrapper = mountSettings();
    await flushPromises();
    expect(wrapper.text()).toContain("CLOUDFLARE_API_TOKEN");
    expect(wrapper.text()).toContain("Stored");
  });

  it("displays cert info", async () => {
    const wrapper = mountSettings();
    await flushPromises();
    expect(wrapper.text()).toContain("Let's Encrypt");
    expect(wrapper.text()).toContain("*.test.dev");
  });

  it("displays version", async () => {
    const wrapper = mountSettings();
    await flushPromises();
    expect(wrapper.text()).toContain("Pulse Gateway v");
  });

  it("shows env vars as Configured in cert section", async () => {
    const wrapper = mountSettings();
    await flushPromises();
    expect(wrapper.text()).toContain("Configured");
  });

  it("shows Missing badge when env var has no value", async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      switch (cmd) {
        case "get_settings":
          return { domain: "test.dev", caddy_image: "caddy:2", dns_provider: "cloudflare" };
        case "get_env_vars":
          return [["SOME_TOKEN", false]];
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
        default:
          return {};
      }
    });

    const wrapper = mountSettings();
    await flushPromises();
    expect(wrapper.text()).toContain("SOME_TOKEN");
    expect(wrapper.text()).toContain("Missing");
  });

  it("shows None badge when no env vars configured", async () => {
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
        default:
          return {};
      }
    });

    const wrapper = mountSettings();
    await flushPromises();
    expect(wrapper.text()).toContain("None");
    expect(wrapper.text()).toContain("No environment variables configured");
  });

  it("shows cert error message", async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      switch (cmd) {
        case "get_settings":
          return { domain: "test.dev", caddy_image: "caddy:2", dns_provider: "cloudflare" };
        case "get_env_vars":
          return [];
        case "get_cert_info":
          return {
            has_env_vars: false,
            domain: "*.test.dev",
            issuer: null,
            not_before: null,
            not_after: null,
            subject_alt_names: null,
            error: "No valid certificates found yet.",
          };
        default:
          return {};
      }
    });

    const wrapper = mountSettings();
    await flushPromises();
    expect(wrapper.text()).toContain("No valid certificates found yet.");
  });

  it("shows Caddy image hint for cloudflare", async () => {
    const wrapper = mountSettings();
    await flushPromises();
    expect(wrapper.text()).toContain("Cloudflare DNS plugin");
  });

  it("shows auto-renew note", async () => {
    const wrapper = mountSettings();
    await flushPromises();
    expect(wrapper.text()).toContain("auto-renews");
  });

  it("has Refresh button for cert info", async () => {
    const wrapper = mountSettings();
    await flushPromises();
    const refreshBtn = wrapper.findAll("button").find((b) => b.text() === "Refresh");
    expect(refreshBtn).toBeDefined();
  });

  it("has Remove button for env vars", async () => {
    const wrapper = mountSettings();
    await flushPromises();
    const removeBtn = wrapper.findAll("button").find((b) => b.text() === "Remove");
    expect(removeBtn).toBeDefined();
  });

  it("has Add button for env vars", async () => {
    const wrapper = mountSettings();
    await flushPromises();
    const addBtn = wrapper.findAll("button").find((b) => b.text() === "Add");
    expect(addBtn).toBeDefined();
  });

  it("renders subdomain hint with domain", async () => {
    const wrapper = mountSettings();
    await flushPromises();
    expect(wrapper.text()).toContain("*.test.dev");
  });
});
