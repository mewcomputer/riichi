import { afterEach, describe, expect, it, vi } from "vitest";

import {
  ApiError,
  applyDocumentLoroUpdate,
  getApiBaseUrl,
  getDocumentLoroSnapshot,
  getProjectQueue,
  importGithubIssues,
  revokeAgentSession,
  setApiBaseUrl,
} from "./api";

afterEach(() => {
  vi.restoreAllMocks();
});

describe("project queue API", () => {
  it("sends cookies and returns the server issue list", async () => {
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ issues: [{ display_key: "RII-1" }] }), {
        status: 200,
        headers: { "content-type": "application/json" },
      }),
    );
    vi.stubGlobal("fetch", fetchMock);

    await expect(getProjectQueue("project/id")).resolves.toEqual([
      { display_key: "RII-1" },
    ]);
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/v1/projects/project%2Fid/queue",
      expect.objectContaining({
        credentials: "include",
        headers: { Accept: "application/json" },
      }),
    );
  });

  it("preserves structured API errors for the UI", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        new Response(JSON.stringify({ code: "human_unauthenticated", message: "sign in required" }), {
          status: 401,
          headers: { "content-type": "application/json" },
        }),
      ),
    );

    await expect(getProjectQueue("project-id")).rejects.toEqual(
      new ApiError(401, "sign in required", "human_unauthenticated"),
    );
  });

  it("falls back to the HTTP status when an error response is not JSON", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response("upstream unavailable", { status: 503 })));

    await expect(getProjectQueue("project-id")).rejects.toMatchObject({
      status: 503,
      message: "Request failed with status 503",
    });
  });
});

describe("operator API", () => {
  it("posts a Loro update as a bounded attributed command", async () => {
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ update_id: "update-1", replayed: false }), {
        status: 200,
        headers: { "content-type": "application/json" },
      }),
    );
    vi.stubGlobal("fetch", fetchMock);

    await expect(
      applyDocumentLoroUpdate("document/id", {
        update_id: "update-1",
        idempotency_key: "edit-1",
        previous_frontiers: [{ peer_id: "42", counter: 9 }],
        payload: new Uint8Array([1, 2, 3]),
      }),
    ).resolves.toMatchObject({ replayed: false });
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/v1/documents/document%2Fid/loro-updates",
      expect.objectContaining({
        method: "POST",
        credentials: "include",
        body: JSON.stringify({
          schema_version: 2,
          update_id: "update-1",
          idempotency_key: "edit-1",
          previous_frontiers: [{ peer_id: "42", counter: 9 }],
          payload_base64: "AQID",
        }),
      }),
    );
  });

  it("loads a binary Loro snapshot with revision metadata", async () => {
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(new Uint8Array([1, 2, 3]), {
        status: 200,
        headers: {
          "content-type": "application/octet-stream",
          "x-riichi-document-revision": "4",
          "x-riichi-document-schema-version": "1",
          "x-riichi-document-frontiers": JSON.stringify([{ peer: "42", counter: 9 }]),
        },
      }),
    );
    vi.stubGlobal("fetch", fetchMock);

    const result = await getDocumentLoroSnapshot("document/id", 4);
    expect(result).toMatchObject({
      revision: 4,
      schema_version: 1,
      frontiers: [{ peer: "42", counter: 9 }],
    });
    expect(Array.from(new Uint8Array(result.bytes))).toEqual([1, 2, 3]);
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/v1/documents/document%2Fid/loro-snapshot?revision=4",
      { credentials: "include" },
    );
  });

  it("posts a bounded GitHub import request to the project route", async () => {
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ repository: "acme/riichi", imported: 2 }), {
        status: 200,
        headers: { "content-type": "application/json" },
      }),
    );
    vi.stubGlobal("fetch", fetchMock);

    await expect(importGithubIssues("project-id", { repository: "acme/riichi", max_issues: 25 })).resolves.toMatchObject({ imported: 2 });
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/v1/projects/project-id/integrations/github/import",
      expect.objectContaining({
        method: "POST",
        credentials: "include",
        body: JSON.stringify({ repository: "acme/riichi", max_issues: 25 }),
      }),
    );
  });

  it("handles empty successful responses for revoke controls", async () => {
    const fetchMock = vi.fn().mockResolvedValue(new Response(null, { status: 204 }));
    vi.stubGlobal("fetch", fetchMock);

    await expect(revokeAgentSession("project-id", "session-id")).resolves.toBeUndefined();
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/v1/projects/project-id/agent-sessions/session-id/revoke",
      expect.objectContaining({ method: "POST", credentials: "include" }),
    );
  });
});

describe("runtime API base URL", () => {
  afterEach(() => {
    localStorage.clear();
    vi.unstubAllGlobals();
  });

  it("returns the localStorage override when set", () => {
    localStorage.setItem("riichi:api-base-url", "https://api.example.com");
    expect(getApiBaseUrl()).toBe("https://api.example.com");
  });

  it("falls back to empty string when localStorage is empty", () => {
    expect(getApiBaseUrl()).toBe("");
  });

  it("persists the URL and reloads the page", () => {
    const reloadMock = vi.fn();
    vi.stubGlobal("location", { reload: reloadMock });
    setApiBaseUrl("https://staging.example.com");
    expect(localStorage.getItem("riichi:api-base-url")).toBe("https://staging.example.com");
    expect(reloadMock).toHaveBeenCalledTimes(1);
  });

  it("strips trailing slashes before persisting", () => {
    const reloadMock = vi.fn();
    vi.stubGlobal("location", { reload: reloadMock });
    setApiBaseUrl("https://staging.example.com/");
    expect(localStorage.getItem("riichi:api-base-url")).toBe("https://staging.example.com");
  });

  it("removes the override when set to empty string", () => {
    localStorage.setItem("riichi:api-base-url", "https://old.example.com");
    const reloadMock = vi.fn();
    vi.stubGlobal("location", { reload: reloadMock });
    setApiBaseUrl("");
    expect(localStorage.getItem("riichi:api-base-url")).toBeNull();
  });

  it("routes fetch calls through the override URL", async () => {
    localStorage.setItem("riichi:api-base-url", "https://api.example.com");
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ issues: [] }), {
        status: 200,
        headers: { "content-type": "application/json" },
      }),
    );
    vi.stubGlobal("fetch", fetchMock);
    await getProjectQueue("project-id");
    expect(fetchMock).toHaveBeenCalledWith(
      "https://api.example.com/api/v1/projects/project-id/queue",
      expect.objectContaining({ credentials: "include" }),
    );
  });
});
