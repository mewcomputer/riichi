import { describe, expect, it } from "vitest";

import { advanceShortcut } from "./keyboard-shortcuts";

describe("keyboard shortcut sequences", () => {
  const sequences = [["g", "i"], ["g", "b"], ["g", "m"], ["f", "r"], ["c"]];

  it("keeps valid prefixes and resolves complete sequences", () => {
    expect(advanceShortcut([], "g", sequences)).toEqual({ buffer: ["g"] });
    expect(advanceShortcut(["g"], "b", sequences)).toEqual({ buffer: [], matched: "g b" });
    expect(advanceShortcut(["g"], "m", sequences)).toEqual({ buffer: [], matched: "g m" });
    expect(advanceShortcut([], "c", sequences)).toEqual({ buffer: [], matched: "c" });
  });

  it("drops invalid sequences without producing a command", () => {
    expect(advanceShortcut(["g"], "x", sequences)).toEqual({ buffer: [] });
  });
});
