import { describe, expect, it } from "vitest";
import { Type } from "lucide-react";

import { extractDocumentReferences, filterResourceItems } from "./document-references";
import { filterMentionItems, filterSlashCommandItems } from "./editor-suggestions";

describe("document editor suggestions", () => {
  it("filters mentions by display name and description", () => {
    const items = [
      { id: "1", label: "Natalie", description: "natalie@example.com" },
      { id: "2", label: "Sam", description: "sam@example.com" },
    ];

    expect(filterMentionItems(items, "sam")).toEqual([items[1]]);
    expect(filterMentionItems(items, "example.com")).toEqual(items);
  });

  it("filters slash commands and caps the menu size", () => {
    const items = Array.from({ length: 10 }, (_, index) => ({
      id: `command-${index}`,
      label: `Heading ${index}`,
      description: "Turn the current block into a heading",
      icon: Type,
      command: () => undefined,
    }));

    expect(filterSlashCommandItems(items, "heading")).toHaveLength(8);
    expect(filterSlashCommandItems(items, "nothing")).toEqual([]);
  });

  it("filters resource links by label, description, or kind", () => {
    const items = [
      { id: "issue-1", label: "RII-1 · Sync", description: "RII project", kind: "issue" as const, href: "/riichi/issues/1" },
      { id: "team-1", label: "Platform", description: "PLT team", kind: "team" as const, href: "/riichi/teams/PLT" },
    ];

    expect(filterResourceItems(items, "platform")).toEqual([items[1]]);
    expect(filterResourceItems(items, "team")).toEqual([items[1]]);
  });

  it("extracts and deduplicates inline resource references from document JSON", () => {
    expect(extractDocumentReferences({
      type: "doc",
      content: [{
        type: "paragraph",
        content: [{
          type: "text",
          text: "RII-1",
          marks: [{
            type: "link",
            attrs: { resourceKind: "issue", resourceId: "issue-1", sourceBlockId: "block-1" },
          }],
        }, {
          type: "text",
          text: " again",
          marks: [{
            type: "link",
            attrs: { resourceKind: "issue", resourceId: "issue-1", sourceBlockId: "block-1" },
          }],
        }],
      }],
    })).toEqual([{
      source_block_id: "block-1",
      resource_kind: "issue",
      resource_id: "issue-1",
      reference_kind: "inline",
    }]);
  });
});
