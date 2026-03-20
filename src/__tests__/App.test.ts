import { describe, it, expect, vi } from "vitest";
import { mount } from "@vue/test-utils";
import { createRouter, createWebHistory } from "vue-router";
import App from "../App.vue";

vi.mock("@tauri-apps/api/core");
vi.mock("@tauri-apps/api/event");
vi.mock("@tauri-apps/plugin-opener");

const router = createRouter({
  history: createWebHistory(),
  routes: [
    { path: "/", component: { template: "<div>Dashboard</div>" } },
    { path: "/settings", component: { template: "<div>Settings</div>" } },
  ],
});

describe("App.vue", () => {
  it("renders the nav brand", async () => {
    router.push("/");
    await router.isReady();

    const wrapper = mount(App, {
      global: { plugins: [router] },
    });

    expect(wrapper.find(".nav-brand").text()).toContain("Pulse Gateway");
  });

  it("renders the logo", async () => {
    router.push("/");
    await router.isReady();

    const wrapper = mount(App, {
      global: { plugins: [router] },
    });

    expect(wrapper.find(".nav-logo").exists()).toBe(true);
  });

  it("renders dashboard and settings nav links", async () => {
    router.push("/");
    await router.isReady();

    const wrapper = mount(App, {
      global: { plugins: [router] },
    });

    const links = wrapper.findAll(".nav-links a");
    expect(links).toHaveLength(2);
    expect(links[0].text()).toBe("Dashboard");
    expect(links[1].text()).toBe("Settings");
  });

  it("has correct href on settings link", async () => {
    router.push("/");
    await router.isReady();

    const wrapper = mount(App, {
      global: { plugins: [router] },
    });

    const settingsLink = wrapper.findAll(".nav-links a")[1];
    expect(settingsLink.attributes("href")).toBe("/settings");
  });
});
