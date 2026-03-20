import { describe, it, expect } from "vitest";
import { gatewayStatus, statusLabel, fqdn, gatewayUrl } from "../gateway-helpers";

describe("gatewayStatus", () => {
  it("returns ssl when TLS configured and cert ready", () => {
    expect(gatewayStatus(true, true)).toBe("ssl");
  });

  it("returns proxy when no TLS", () => {
    expect(gatewayStatus(false, false)).toBe("proxy");
  });

  it("returns proxy when TLS configured but cert not ready", () => {
    expect(gatewayStatus(true, false)).toBe("proxy");
  });

  it("returns proxy when cert ready but no TLS config", () => {
    expect(gatewayStatus(false, true)).toBe("proxy");
  });
});

describe("statusLabel", () => {
  it("maps ssl to SSL", () => {
    expect(statusLabel("ssl")).toBe("SSL");
  });

  it("maps proxy to Proxy", () => {
    expect(statusLabel("proxy")).toBe("Proxy");
  });

  it("passes through unknown status", () => {
    expect(statusLabel("unknown")).toBe("unknown");
  });
});

describe("fqdn", () => {
  it("returns subdomain.domain when domain is set", () => {
    expect(fqdn("app", "example.com")).toBe("app.example.com");
  });

  it("returns just subdomain when domain is empty", () => {
    expect(fqdn("app", "")).toBe("app");
  });
});

describe("gatewayUrl", () => {
  it("returns https URL when TLS is enabled", () => {
    expect(gatewayUrl("app", "example.com", true)).toBe("https://app.example.com");
  });

  it("returns http URL when TLS is disabled", () => {
    expect(gatewayUrl("app", "example.com", false)).toBe("http://app.example.com");
  });

  it("works without domain", () => {
    expect(gatewayUrl("app", "", true)).toBe("https://app");
  });

  it("handles subdomain with hyphens", () => {
    expect(gatewayUrl("my-cool-app", "dev.local", true)).toBe("https://my-cool-app.dev.local");
  });

  it("handles multi-level domain", () => {
    expect(gatewayUrl("app", "sub.example.co.uk", false)).toBe("http://app.sub.example.co.uk");
  });
});

describe("fqdn edge cases", () => {
  it("handles hyphenated subdomain", () => {
    expect(fqdn("my-app-2", "test.dev")).toBe("my-app-2.test.dev");
  });

  it("handles numeric subdomain", () => {
    expect(fqdn("12345", "test.dev")).toBe("12345.test.dev");
  });
});

describe("statusLabel edge cases", () => {
  it("returns empty string for empty input", () => {
    expect(statusLabel("")).toBe("");
  });

  it("is case-sensitive", () => {
    expect(statusLabel("SSL")).toBe("SSL"); // passthrough, not mapped
    expect(statusLabel("Proxy")).toBe("Proxy"); // passthrough
  });
});
