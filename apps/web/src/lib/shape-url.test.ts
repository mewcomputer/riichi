import { describe, expect, it } from "vitest";

import { authenticatedShapeUrl } from "./shape-url";

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
});
