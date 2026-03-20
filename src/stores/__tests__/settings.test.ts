import { describe, it, expect, vi, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";

vi.mock("@tauri-apps/api/core");

import { invoke } from "@tauri-apps/api/core";
import { useSettingsStore } from "../settings";

const mockedInvoke = vi.mocked(invoke);

describe("settings store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
  });

  describe("fetchSettings", () => {
    it("populates domain, caddyImage, and dnsProvider", async () => {
      mockedInvoke.mockResolvedValueOnce({
        domain: "example.com",
        caddy_image: "caddy:2",
        dns_provider: "cloudflare",
      });

      const store = useSettingsStore();
      await store.fetchSettings();

      expect(store.domain).toBe("example.com");
      expect(store.caddyImage).toBe("caddy:2");
      expect(store.dnsProvider).toBe("cloudflare");
    });

    it("handles error gracefully", async () => {
      mockedInvoke.mockRejectedValueOnce(new Error("fail"));

      const store = useSettingsStore();
      await store.fetchSettings();

      // Should not throw, values stay at defaults
      expect(store.domain).toBe("");
    });
  });

  describe("saveSettings", () => {
    it("sets saving flag during save", async () => {
      mockedInvoke.mockResolvedValueOnce({
        domain: "new.com",
        caddy_image: "caddy:2",
        dns_provider: "porkbun",
      });

      const store = useSettingsStore();
      const promise = store.saveSettings();

      expect(store.saving).toBe(true);
      await promise;
      expect(store.saving).toBe(false);
    });

    it("sends current values to backend", async () => {
      mockedInvoke.mockResolvedValueOnce({
        domain: "test.com",
        caddy_image: "caddy:2",
        dns_provider: "cloudflare",
      });

      const store = useSettingsStore();
      store.domain = "test.com";
      store.caddyImage = "caddy:2";
      store.dnsProvider = "cloudflare";
      await store.saveSettings();

      expect(mockedInvoke).toHaveBeenCalledWith("save_settings", {
        domain: "test.com",
        caddyImage: "caddy:2",
        dnsProvider: "cloudflare",
      });
    });
  });

  describe("fetchEnvVars", () => {
    it("populates envVars array", async () => {
      const vars: [string, boolean][] = [
        ["CLOUDFLARE_API_TOKEN", true],
        ["OTHER_KEY", false],
      ];
      mockedInvoke.mockResolvedValueOnce(vars);

      const store = useSettingsStore();
      await store.fetchEnvVars();

      expect(store.envVars).toEqual(vars);
      expect(store.envVars.length).toBe(2);
    });
  });

  describe("saveEnvVar", () => {
    it("updates envVars with result", async () => {
      const updated: [string, boolean][] = [["API_KEY", true]];
      mockedInvoke.mockResolvedValueOnce(updated);

      const store = useSettingsStore();
      await store.saveEnvVar("API_KEY", "secret");

      expect(mockedInvoke).toHaveBeenCalledWith("save_env_var", {
        key: "API_KEY",
        value: "secret",
      });
      expect(store.envVars).toEqual(updated);
    });

    it("sets savingEnv flag", async () => {
      mockedInvoke.mockResolvedValueOnce([]);

      const store = useSettingsStore();
      const promise = store.saveEnvVar("KEY", "val");

      expect(store.savingEnv).toBe(true);
      await promise;
      expect(store.savingEnv).toBe(false);
    });
  });

  describe("removeEnvVar", () => {
    it("calls invoke and updates envVars", async () => {
      const updated: [string, boolean][] = [];
      mockedInvoke.mockResolvedValueOnce(updated);

      const store = useSettingsStore();
      await store.removeEnvVar("API_KEY");

      expect(mockedInvoke).toHaveBeenCalledWith("remove_env_var", { key: "API_KEY" });
      expect(store.envVars).toEqual([]);
    });
  });

  describe("fetchCertInfo", () => {
    it("populates certInfo", async () => {
      const cert = {
        has_env_vars: true,
        domain: "*.example.com",
        issuer: "Let's Encrypt",
        not_before: null,
        not_after: null,
        subject_alt_names: "app.example.com",
        error: null,
      };
      mockedInvoke.mockResolvedValueOnce(cert);

      const store = useSettingsStore();
      await store.fetchCertInfo();

      expect(store.certInfo.issuer).toBe("Let's Encrypt");
      expect(store.certInfo.has_env_vars).toBe(true);
    });

    it("handles error gracefully", async () => {
      mockedInvoke.mockRejectedValueOnce(new Error("fail"));

      const store = useSettingsStore();
      await store.fetchCertInfo();

      // Should not throw, certInfo stays at defaults
      expect(store.certInfo.has_env_vars).toBe(false);
    });
  });

  describe("initial state", () => {
    it("starts with empty domain", () => {
      const store = useSettingsStore();
      expect(store.domain).toBe("");
    });

    it("starts with default caddy image", () => {
      const store = useSettingsStore();
      expect(store.caddyImage).toBe("caddy:2");
    });

    it("starts with cloudflare provider", () => {
      const store = useSettingsStore();
      expect(store.dnsProvider).toBe("cloudflare");
    });

    it("starts with empty env vars", () => {
      const store = useSettingsStore();
      expect(store.envVars).toEqual([]);
    });

    it("starts not saving", () => {
      const store = useSettingsStore();
      expect(store.saving).toBe(false);
      expect(store.savingEnv).toBe(false);
    });

    it("starts with default cert info", () => {
      const store = useSettingsStore();
      expect(store.certInfo.has_env_vars).toBe(false);
      expect(store.certInfo.domain).toBeNull();
      expect(store.certInfo.issuer).toBeNull();
    });

    it("starts with null cert error", () => {
      const store = useSettingsStore();
      expect(store.certInfo.error).toBeNull();
      expect(store.certInfo.not_before).toBeNull();
      expect(store.certInfo.not_after).toBeNull();
      expect(store.certInfo.subject_alt_names).toBeNull();
    });
  });

  describe("saveSettings", () => {
    it("updates state from response", async () => {
      mockedInvoke.mockResolvedValueOnce({
        domain: "updated.dev",
        caddy_image: "caddy:custom",
        dns_provider: "porkbun",
      });

      const store = useSettingsStore();
      store.domain = "updated.dev";
      store.caddyImage = "caddy:custom";
      store.dnsProvider = "porkbun";
      await store.saveSettings();

      expect(store.domain).toBe("updated.dev");
      expect(store.caddyImage).toBe("caddy:custom");
      expect(store.dnsProvider).toBe("porkbun");
    });

    it("resets saving on error", async () => {
      mockedInvoke.mockRejectedValueOnce(new Error("save failed"));

      const store = useSettingsStore();
      try {
        await store.saveSettings();
      } catch {
        // expected
      }

      expect(store.saving).toBe(false);
    });
  });

  describe("fetchEnvVars", () => {
    it("handles error gracefully", async () => {
      mockedInvoke.mockRejectedValueOnce(new Error("fail"));

      const store = useSettingsStore();
      await store.fetchEnvVars();

      expect(store.envVars).toEqual([]);
    });
  });

  describe("removeEnvVar", () => {
    it("handles error gracefully", async () => {
      mockedInvoke.mockRejectedValueOnce(new Error("fail"));

      const store = useSettingsStore();
      store.envVars = [["KEY", true]];
      await store.removeEnvVar("KEY");

      // Should not throw, envVars stays unchanged on error
      expect(store.envVars).toEqual([["KEY", true]]);
    });
  });
});
