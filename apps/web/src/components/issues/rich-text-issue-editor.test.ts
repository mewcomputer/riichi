import { describe, expect, it } from "vitest";

import { shouldSyncControlledEditor } from "@/components/issues/rich-text-issue-editor";

describe("shouldSyncControlledEditor", () => {
  it("does not overwrite a local editor value during an unrelated parent rerender", () => {
    expect(shouldSyncControlledEditor("draft", "server", "draft")).toBe(false);
  });

  it("accepts a changed external value when it differs from the editor", () => {
    expect(shouldSyncControlledEditor("server update", "local draft", "server")).toBe(true);
  });

  it("does not issue a redundant setContent when the editor already matches", () => {
    expect(shouldSyncControlledEditor("server update", "server update", "server")).toBe(false);
  });
});
