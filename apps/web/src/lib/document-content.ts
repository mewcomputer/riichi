export function normalizeDocumentContent(value: Record<string, unknown>): Record<string, unknown> {
  if (value.type !== "doc" || !Array.isArray(value.content) || value.content.length === 0) {
    return { type: "doc", content: [{ type: "paragraph" }] };
  }
  return value;
}
