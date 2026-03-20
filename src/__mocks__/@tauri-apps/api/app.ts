import { vi } from "vitest";

export const getVersion = vi.fn().mockResolvedValue("0.1.0-test");
