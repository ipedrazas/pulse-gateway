import { describe, it, expect, vi, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";

vi.mock("@tauri-apps/api/core");
vi.mock("@tauri-apps/api/event");

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useGatewayStore } from "../gateway";

const mockedInvoke = vi.mocked(invoke);
const mockedListen = vi.mocked(listen);

describe("gateway store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
  });

  describe("fetchStatus", () => {
    it("sets caddyStatus on success", async () => {
      const status = { running: true, api_reachable: true, error: null };
      mockedInvoke.mockResolvedValueOnce(status);

      const store = useGatewayStore();
      await store.fetchStatus();

      expect(mockedInvoke).toHaveBeenCalledWith("get_caddy_status");
      expect(store.caddyStatus).toEqual(status);
    });

    it("sets error status on failure", async () => {
      mockedInvoke.mockRejectedValueOnce(new Error("connection failed"));

      const store = useGatewayStore();
      await store.fetchStatus();

      expect(store.caddyStatus.running).toBe(false);
      expect(store.caddyStatus.error).toContain("connection failed");
    });
  });

  describe("startCaddy", () => {
    it("sets loading during start", async () => {
      const status = { running: true, api_reachable: true, error: null };
      mockedInvoke.mockResolvedValue(status);

      const store = useGatewayStore();
      const promise = store.startCaddy();

      expect(store.loading).toBe(true);
      await promise;
      expect(store.loading).toBe(false);
    });

    it("updates caddyStatus on success", async () => {
      const status = { running: true, api_reachable: true, error: null };
      mockedInvoke.mockResolvedValue(status);

      const store = useGatewayStore();
      await store.startCaddy();

      expect(store.caddyStatus.running).toBe(true);
    });

    it("sets error status on failure", async () => {
      mockedInvoke.mockRejectedValueOnce(new Error("docker not running"));

      const store = useGatewayStore();
      await store.startCaddy();

      expect(store.caddyStatus.running).toBe(false);
      expect(store.caddyStatus.error).toContain("docker not running");
      expect(store.loading).toBe(false);
    });
  });

  describe("stopCaddy", () => {
    it("sets loading during stop", async () => {
      const status = { running: false, api_reachable: false, error: null };
      mockedInvoke.mockResolvedValueOnce(status);

      const store = useGatewayStore();
      const promise = store.stopCaddy();

      expect(store.loading).toBe(true);
      await promise;
      expect(store.loading).toBe(false);
    });
  });

  describe("fetchAllGateways", () => {
    it("populates allGateways", async () => {
      const gateways = [{ subdomain: "app", target_host: "c1", port: 80, source: "auto" }];
      mockedInvoke.mockResolvedValueOnce(gateways);

      const store = useGatewayStore();
      await store.fetchAllGateways();

      expect(store.allGateways).toEqual(gateways);
    });
  });

  describe("addRoute", () => {
    it("calls invoke with correct args", async () => {
      mockedInvoke.mockResolvedValueOnce([]);

      const store = useGatewayStore();
      await store.addRoute("myapp", "localhost", 3000);

      expect(mockedInvoke).toHaveBeenCalledWith("add_route", {
        subdomain: "myapp",
        targetHost: "localhost",
        port: 3000,
      });
    });
  });

  describe("removeRoute", () => {
    it("calls invoke with subdomain", async () => {
      mockedInvoke.mockResolvedValueOnce([]);

      const store = useGatewayStore();
      await store.removeRoute("myapp");

      expect(mockedInvoke).toHaveBeenCalledWith("remove_route", {
        subdomain: "myapp",
      });
    });
  });

  describe("fetchStaticRoutes", () => {
    it("populates staticRoutes", async () => {
      const routes = [{ subdomain: "s1", target_host: "h", port: 80, source: "static" }];
      mockedInvoke.mockResolvedValueOnce(routes);

      const store = useGatewayStore();
      await store.fetchStaticRoutes();

      expect(store.staticRoutes).toEqual(routes);
    });
  });

  describe("fetchEventLog", () => {
    it("populates eventLog", async () => {
      const logs = [
        { timestamp: "12:00:00", level: "info", message: "started" },
        { timestamp: "12:00:01", level: "error", message: "oops" },
      ];
      mockedInvoke.mockResolvedValueOnce(logs);

      const store = useGatewayStore();
      await store.fetchEventLog();

      expect(store.eventLog).toHaveLength(2);
      expect(store.eventLog[0].level).toBe("info");
    });
  });

  describe("stopCaddy", () => {
    it("sets error on failure", async () => {
      mockedInvoke.mockRejectedValueOnce(new Error("stop failed"));

      const store = useGatewayStore();
      await store.stopCaddy();

      expect(store.caddyStatus.running).toBe(false);
      expect(store.caddyStatus.error).toContain("stop failed");
      expect(store.loading).toBe(false);
    });
  });

  describe("fetchAllGateways", () => {
    it("handles error gracefully", async () => {
      mockedInvoke.mockRejectedValueOnce(new Error("fail"));

      const store = useGatewayStore();
      await store.fetchAllGateways();

      // Should not throw, stays empty
      expect(store.allGateways).toEqual([]);
    });
  });

  describe("fetchStaticRoutes", () => {
    it("handles error gracefully", async () => {
      mockedInvoke.mockRejectedValueOnce(new Error("fail"));

      const store = useGatewayStore();
      await store.fetchStaticRoutes();

      expect(store.staticRoutes).toEqual([]);
    });
  });

  describe("fetchEventLog", () => {
    it("handles error gracefully", async () => {
      mockedInvoke.mockRejectedValueOnce(new Error("fail"));

      const store = useGatewayStore();
      await store.fetchEventLog();

      expect(store.eventLog).toEqual([]);
    });
  });

  describe("initial state", () => {
    it("starts with empty gateways", () => {
      const store = useGatewayStore();
      expect(store.allGateways).toEqual([]);
      expect(store.staticRoutes).toEqual([]);
    });

    it("starts with caddy not running", () => {
      const store = useGatewayStore();
      expect(store.caddyStatus.running).toBe(false);
      expect(store.caddyStatus.api_reachable).toBe(false);
      expect(store.caddyStatus.error).toBeNull();
    });

    it("starts not loading", () => {
      const store = useGatewayStore();
      expect(store.loading).toBe(false);
    });

    it("starts with empty event log", () => {
      const store = useGatewayStore();
      expect(store.eventLog).toEqual([]);
    });
  });

  describe("init", () => {
    it("calls all fetch functions", async () => {
      mockedInvoke.mockResolvedValue([]);

      const store = useGatewayStore();
      await store.init();

      const commands = mockedInvoke.mock.calls.map((c) => c[0]);
      expect(commands).toContain("get_caddy_status");
      expect(commands).toContain("get_all_gateways");
      expect(commands).toContain("get_routes");
      expect(commands).toContain("get_event_log");
    });
  });

  describe("addRoute", () => {
    it("updates staticRoutes with result", async () => {
      const updated = [
        { subdomain: "myapp", target_host: "localhost", port: 3000, source: "static" },
      ];
      mockedInvoke.mockResolvedValueOnce(updated);

      const store = useGatewayStore();
      await store.addRoute("myapp", "localhost", 3000);

      expect(store.staticRoutes).toEqual(updated);
    });
  });

  describe("removeRoute", () => {
    it("updates staticRoutes with result", async () => {
      mockedInvoke.mockResolvedValueOnce([]);

      const store = useGatewayStore();
      // Prepopulate
      store.staticRoutes = [
        {
          subdomain: "myapp",
          target_host: "localhost",
          port: 3000,
          source: "static" as const,
        },
      ];

      await store.removeRoute("myapp");

      expect(store.staticRoutes).toEqual([]);
    });
  });

  describe("startCaddy", () => {
    it("fetches gateways and routes after start", async () => {
      const status = { running: true, api_reachable: true, error: null };
      mockedInvoke.mockResolvedValue(status);

      const store = useGatewayStore();
      await store.startCaddy();

      const commands = mockedInvoke.mock.calls.map((c) => c[0]);
      expect(commands).toContain("start_caddy");
      expect(commands).toContain("get_all_gateways");
      expect(commands).toContain("get_routes");
    });
  });

  describe("event listeners", () => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    let listenerCallbacks: Record<string, (event: any) => void>;

    beforeEach(() => {
      listenerCallbacks = {};
      mockedListen.mockImplementation(async (eventName: string, callback) => {
        listenerCallbacks[eventName] = callback;
        return () => {};
      });
      mockedInvoke.mockResolvedValue([]);
    });

    it("registers gateways-changed and log-entry listeners on init", async () => {
      const store = useGatewayStore();
      await store.init();

      expect(mockedListen).toHaveBeenCalledWith("gateways-changed", expect.any(Function));
      expect(mockedListen).toHaveBeenCalledWith("log-entry", expect.any(Function));
    });

    it("gateways-changed event updates allGateways", async () => {
      const store = useGatewayStore();
      await store.init();

      const newGateways = [
        { subdomain: "app1", target_host: "c1", port: 80, source: "auto" as const },
        { subdomain: "app2", target_host: "c2", port: 8080, source: "auto" as const },
      ];

      listenerCallbacks["gateways-changed"]({ payload: newGateways });

      expect(store.allGateways).toEqual(newGateways);
      expect(store.allGateways).toHaveLength(2);
    });

    it("log-entry event appends to eventLog", async () => {
      const store = useGatewayStore();
      await store.init();

      listenerCallbacks["log-entry"]({
        payload: { timestamp: "12:00:00", level: "info", message: "test msg" },
      });

      expect(store.eventLog).toHaveLength(1);
      expect(store.eventLog[0].message).toBe("test msg");
    });

    it("log-entry event appends multiple entries", async () => {
      const store = useGatewayStore();
      await store.init();

      listenerCallbacks["log-entry"]({
        payload: { timestamp: "12:00:00", level: "info", message: "first" },
      });
      listenerCallbacks["log-entry"]({
        payload: { timestamp: "12:00:01", level: "warn", message: "second" },
      });
      listenerCallbacks["log-entry"]({
        payload: { timestamp: "12:00:02", level: "error", message: "third" },
      });

      expect(store.eventLog).toHaveLength(3);
      expect(store.eventLog[0].message).toBe("first");
      expect(store.eventLog[2].message).toBe("third");
    });

    it("log-entry trims at MAX_LOG_ENTRIES (200)", async () => {
      const store = useGatewayStore();
      await store.init();

      // Push 210 entries
      for (let i = 0; i < 210; i++) {
        listenerCallbacks["log-entry"]({
          payload: { timestamp: "12:00:00", level: "info", message: `msg-${i}` },
        });
      }

      expect(store.eventLog).toHaveLength(200);
      // Should keep the last 200, so first entry should be msg-10
      expect(store.eventLog[0].message).toBe("msg-10");
      expect(store.eventLog[199].message).toBe("msg-209");
    });

    it("gateways-changed replaces entire list", async () => {
      const store = useGatewayStore();
      await store.init();

      // First update
      listenerCallbacks["gateways-changed"]({
        payload: [{ subdomain: "old", target_host: "c", port: 80, source: "auto" }],
      });
      expect(store.allGateways).toHaveLength(1);

      // Second update replaces
      listenerCallbacks["gateways-changed"]({
        payload: [
          { subdomain: "new1", target_host: "c1", port: 80, source: "auto" },
          { subdomain: "new2", target_host: "c2", port: 8080, source: "auto" },
        ],
      });
      expect(store.allGateways).toHaveLength(2);
      expect(store.allGateways[0].subdomain).toBe("new1");
    });

    it("does not register duplicate listeners on multiple init calls", async () => {
      const store = useGatewayStore();
      await store.init();
      await store.init();

      // listen should only be called twice total (once for each event), not four times
      const listenCalls = mockedListen.mock.calls.map((c) => c[0]);
      const gatewaysCalls = listenCalls.filter((c) => c === "gateways-changed");
      const logCalls = listenCalls.filter((c) => c === "log-entry");
      expect(gatewaysCalls).toHaveLength(1);
      expect(logCalls).toHaveLength(1);
    });
  });
});
