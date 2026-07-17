/** Electric's ShapeStream requires an absolute URL even when the API proxy is same-origin. */
export function authenticatedShapeUrl(path: string) {
  return new URL(path, globalThis.location.origin).toString();
}
