import { describe, it, expect } from "vitest";
import router from "../index";

describe("router", () => {
  it("has a dashboard route at /", () => {
    const route = router.getRoutes().find((r) => r.path === "/");
    expect(route).toBeDefined();
    expect(route?.name).toBe("dashboard");
  });

  it("has a settings route at /settings", () => {
    const route = router.getRoutes().find((r) => r.path === "/settings");
    expect(route).toBeDefined();
    expect(route?.name).toBe("settings");
  });

  it("has exactly 2 routes", () => {
    expect(router.getRoutes()).toHaveLength(2);
  });
});
