import { getApiBaseUrl } from "./api";

/**
 * Electric's ShapeStream requires an absolute URL even when the API proxy is
 * same-origin. Resolves relative paths against the configured API base URL
 * (which may be a cross-origin override set at runtime), falling back to the
 * page origin when no override is set.
 */
export function authenticatedShapeUrl(path: string) {
  const base = getApiBaseUrl() || globalThis.location.origin;
  return new URL(path, base).toString();
}
