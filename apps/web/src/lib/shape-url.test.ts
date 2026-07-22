import { afterEach, describe, expect, it, vi } from "vitest";

import { authenticatedShapeUrl } from "./shape-url";

afterEach(() => {
  localStorage.clear();
});

describe("authenticatedShapeUrl", () => {
  it("resolves a same-origin API path to an absolute URL", () => {
    expect(authenticatedShapeUrl("/api/v1/sync/navigation")).toBe(
      "http://localhost:3000/api/v1/sync/navigation",
    );
  });

  it("preserves an already absolute shape URL", () => {
    expect(authenticatedShapeUrl("https://electric.example.test/v1/shape")).toBe(
      "https://electric.example.test/v1/shape",
    );
  });

  it("resolves against a runtime API base URL override", () => {
    localStorage.setItem("riichi:api-base-url", "https://api.staging.example.com");
    expect(authenticatedShapeUrl("/api/v1/sync/navigation")).toBe(
      "https://api.staging.example.com/api/v1/sync/navigation",
    );
  });

  it("strips trailing slashes from the override before resolving", () => {
    localStorage.setItem("riichi:api-base-url", "https://api.staging.example.com/");
    expect(authenticatedShapeUrl("/api/v1/sync/issues")).toBe(
      "https://api.staging.example.com/api/v1/sync/issues",
    );
  });
});
