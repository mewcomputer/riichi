import { describe, expect, it } from "vitest";

import { shortcutReference } from "./command-menu";

describe("shortcut reference", () => {
  it("documents the queue navigation and command entry points", () => {
    expect(shortcutReference).toEqual(expect.arrayContaining([
      ["⌘ K / Ctrl K", "Open command menu"],
      ["G I", "Open all issues"],
      ["G M", "Open my work"],
      ["J / K", "Move through queue issues"],
      ["Enter", "Open the selected issue"],
    ]));
  });
});
