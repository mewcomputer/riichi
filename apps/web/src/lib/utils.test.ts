import { describe, expect, it } from "vitest";

import { formatRelativeTime } from "./utils";

describe("formatRelativeTime", () => {
  const now = new Date("2026-07-15T12:00:00.000Z");

  it("uses a compact seconds label for recent edits", () => {
    expect(formatRelativeTime("2026-07-15T11:59:50.000Z", now)).toBe("10s ago");
  });

  it("uses the next largest unit as an edit ages", () => {
    expect(formatRelativeTime("2026-07-15T11:58:00.000Z", now)).toBe("2m ago");
    expect(formatRelativeTime("2026-07-15T10:00:00.000Z", now)).toBe("2h ago");
    expect(formatRelativeTime("2026-07-13T12:00:00.000Z", now)).toBe("2d ago");
  });

  it("does not report a future edit as stale", () => {
    expect(formatRelativeTime("2026-07-15T12:00:30.000Z", now)).toBe("just now");
  });

  it("falls back safely for invalid timestamps", () => {
    expect(formatRelativeTime("not a timestamp", now)).toBe("unknown");
  });
});
