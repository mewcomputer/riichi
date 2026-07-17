import { describe, expect, it } from "vitest";

import { normalizeDocumentContent } from "./document-content";

describe("normalizeDocumentContent", () => {
  it("gives an empty document a valid paragraph root", () => {
    expect(normalizeDocumentContent({ type: "doc", content: [] })).toEqual({
      type: "doc",
      content: [{ type: "paragraph" }],
    });
  });

  it("preserves existing document blocks", () => {
    const content = {
      type: "doc",
      content: [{ type: "heading", attrs: { level: 2 }, content: [{ type: "text", text: "Title" }] }],
    };

    expect(normalizeDocumentContent(content)).toBe(content);
  });
});
