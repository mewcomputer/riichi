import { describe, expect, it, vi } from "vitest";

import { clearSessionCollections, registerSessionCollection } from "./session-state";

describe("session collection cleanup", () => {
  it("cleans every registered collection and forgets the registry", async () => {
    const first = { cleanup: vi.fn(async () => undefined) };
    const second = { cleanup: vi.fn(async () => undefined) };
    registerSessionCollection(first);
    registerSessionCollection(second);

    await clearSessionCollections();

    expect(first.cleanup).toHaveBeenCalledOnce();
    expect(second.cleanup).toHaveBeenCalledOnce();
    await clearSessionCollections();
    expect(first.cleanup).toHaveBeenCalledOnce();
  });

  it("continues cleaning other collections when one cleanup fails", async () => {
    const failed = { cleanup: vi.fn(async () => { throw new Error("closed"); }) };
    const healthy = { cleanup: vi.fn(async () => undefined) };
    registerSessionCollection(failed);
    registerSessionCollection(healthy);

    await expect(clearSessionCollections()).resolves.toBeUndefined();
    expect(healthy.cleanup).toHaveBeenCalledOnce();
  });
});
