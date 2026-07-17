import type { DocumentReference } from "@/lib/api";
import type { ResourceLinkItem } from "./resource-list";

export function filterResourceItems(items: ResourceLinkItem[], query: string) {
  const normalizedQuery = query.trim().toLocaleLowerCase();
  return items
    .filter((item) => `${item.label} ${item.description ?? ""} ${item.kind}`.toLocaleLowerCase().includes(normalizedQuery))
    .slice(0, 8);
}

export function extractDocumentReferences(content: Record<string, unknown>): Array<{
  source_block_id: string;
  resource_kind: DocumentReference["resource_kind"];
  resource_id: string;
  reference_kind: "inline";
}> {
  const references = new Map<string, {
    source_block_id: string;
    resource_kind: DocumentReference["resource_kind"];
    resource_id: string;
    reference_kind: "inline";
  }>();

  const visit = (value: unknown, path: string) => {
    if (Array.isArray(value)) {
      value.forEach((child, index) => visit(child, `${path}.${index}`));
      return;
    }
    if (!value || typeof value !== "object") return;
    const node = value as Record<string, unknown>;
    const marks = Array.isArray(node.marks) ? node.marks : [];
    for (const mark of marks) {
      if (!mark || typeof mark !== "object") continue;
      const markRecord = mark as Record<string, unknown>;
      if (markRecord.type !== "link" || !markRecord.attrs || typeof markRecord.attrs !== "object") continue;
      const attrs = markRecord.attrs as Record<string, unknown>;
      const resourceKind = attrs.resourceKind ?? attrs.resource_kind;
      const resourceId = attrs.resourceId ?? attrs.resource_id;
      if (!isResourceKind(resourceKind) || typeof resourceId !== "string" || !resourceId) continue;
      const sourceBlockId = typeof (attrs.sourceBlockId ?? attrs.source_block_id) === "string"
        ? String(attrs.sourceBlockId ?? attrs.source_block_id)
        : `resource-link-${path}`;
      const reference = {
        source_block_id: sourceBlockId,
        resource_kind: resourceKind,
        resource_id: resourceId,
        reference_kind: "inline" as const,
      };
      references.set(`${resourceKind}:${resourceId}:${sourceBlockId}`, reference);
    }
    Object.entries(node).forEach(([key, child]) => {
      if (key !== "marks") visit(child, `${path}.${key}`);
    });
  };

  visit(content, "root");
  return [...references.values()];
}

function isResourceKind(value: unknown): value is DocumentReference["resource_kind"] {
  return value === "issue" || value === "team" || value === "project" || value === "document";
}
