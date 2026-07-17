import { afterEach, describe, expect, it, vi } from "vitest";
import { Editor } from "@tiptap/core";
import Image from "@tiptap/extension-image";
import Mention from "@tiptap/extension-mention";
import StarterKit from "@tiptap/starter-kit";
import TaskItem from "@tiptap/extension-task-item";
import TaskList from "@tiptap/extension-task-list";

import { DocumentCallout } from "@/components/documents/document-editor";
import {
  LoroDocumentSession,
  createLoroEditorPlugins,
} from "@/lib/loro-document";
import { LoroDocumentPersistence } from "@/lib/loro-persistence";

describe("LoroDocumentSession", () => {
  afterEach(() => vi.useRealTimers());

  it("supports v2 callout blocks in the editor schema", () => {
    const editor = new Editor({ extensions: [StarterKit, DocumentCallout] });
    editor.commands.setContent({
      type: "doc",
      content: [{
        type: "callout",
        attrs: { kind: "warning" },
        content: [{ type: "paragraph", content: [{ type: "text", text: "careful" }] }],
      }],
    }, { emitUpdate: false });

    expect(editor.getJSON().content?.[0]).toMatchObject({
      type: "callout",
      attrs: { kind: "warning" },
    });
    expect(editor.getHTML()).toContain('data-callout="warning"');
    editor.destroy();
  });

  class FakeWebSocket {
    readyState = 0;
    sent: Array<string | Uint8Array> = [];
    onopen: ((event: Event) => void) | null = null;
    onmessage: ((event: MessageEvent) => void) | null = null;
    onerror: ((event: Event) => void) | null = null;
    onclose: ((event: CloseEvent) => void) | null = null;

    send(data: string | ArrayBufferLike | Blob | ArrayBufferView) {
      this.sent.push(typeof data === "string" ? data : new Uint8Array(data as ArrayBuffer));
    }

    close() {
      this.readyState = 3;
      this.onclose?.(new CloseEvent("close"));
    }

    open() {
      this.readyState = 1;
      this.onopen?.(new Event("open"));
    }

    deliver(data: string | ArrayBuffer) {
      this.onmessage?.(new MessageEvent("message", { data }));
    }
  }

  it("mounts the current Tiptap schema through the Loro binding", () => {
    const session = new LoroDocumentSession();
    const content = {
      type: "doc",
      content: [
        { type: "heading", attrs: { level: 2 }, content: [{ type: "text", text: "Heading" }] },
        { type: "paragraph", content: [
          { type: "text", text: "styled", marks: [{ type: "bold" }, { type: "strike" }] },
          { type: "hardBreak" },
          { type: "mention", attrs: { id: "account-1", label: "Alex" } },
        ] },
        { type: "bulletList", content: [{ type: "listItem", content: [{ type: "paragraph", content: [{ type: "text", text: "bullet" }] }] }] },
        { type: "taskList", content: [{ type: "taskItem", attrs: { checked: true }, content: [{ type: "paragraph", content: [{ type: "text", text: "done" }] }] }] },
        { type: "image", attrs: { src: "/api/attachments/att-1", alt: "diagram", attachmentId: "att-1" } },
      ],
    };
    const AttachmentImage = Image.extend({
      addAttributes() {
        return { ...this.parent?.(), attachmentId: { default: null } };
      },
    });
    const editor = new Editor({
      extensions: [
        StarterKit.configure({ link: false }),
        TaskList,
        TaskItem.configure({ nested: true }),
        Mention,
        AttachmentImage,
      ],
      content,
    });
    for (const plugin of createLoroEditorPlugins(session)) editor.registerPlugin(plugin);

    expect(editor.getJSON()).toMatchObject({
      type: "doc",
      content: expect.arrayContaining([
        expect.objectContaining({ type: "taskList" }),
        expect.objectContaining({ type: "image" }),
      ]),
    });
    editor.destroy();
  });

  it("keeps block-shaped paste content and undo inside the Loro boundary", () => {
    const session = new LoroDocumentSession();
    const editor = new Editor({
      extensions: [StarterKit.configure({ link: false }), TaskList, TaskItem.configure({ nested: true })],
      content: { type: "doc", content: [{ type: "paragraph", content: [{ type: "text", text: "before" }] }] },
    });
    for (const plugin of createLoroEditorPlugins(session)) editor.registerPlugin(plugin);

    editor.commands.insertContent({
      type: "bulletList",
      content: [{ type: "listItem", content: [{ type: "paragraph", content: [{ type: "text", text: "pasted" }] }] }],
    });
    expect(editor.getJSON().content).toEqual(expect.arrayContaining([
      expect.objectContaining({ type: "bulletList" }),
    ]));

    editor.commands.undo();
    expect(editor.getJSON().content).not.toEqual(expect.arrayContaining([
      expect.objectContaining({ type: "bulletList" }),
    ]));
    editor.destroy();
  });

  it("replicates local updates and restores snapshots", () => {
    const writer = new LoroDocumentSession();
    const replica = new LoroDocumentSession();
    const stop = writer.subscribeLocalUpdates((update) => replica.applyUpdate(update));
    const text = writer.doc.getText("text");

    text.insert(0, "hello");
    writer.doc.commit();
    expect(replica.doc.getText("text").toString()).toBe("hello");
    expect(replica.frontierCount()).toBeGreaterThan(0);

    const restored = new LoroDocumentSession(writer.exportSnapshot());
    expect(restored.doc.getText("text").toString()).toBe("hello");
    stop();
  });

  it("rejects empty updates and exposes the editor plugin boundary", () => {
    const session = new LoroDocumentSession();

    expect(() => session.applyUpdate(new Uint8Array())).toThrow(
      "Loro update payload must not be empty",
    );
    expect(createLoroEditorPlugins(session)).toHaveLength(2);
  });

  it("persists a snapshot and restores it in the same document scope", async () => {
    const persistence = new LoroDocumentPersistence(`riichi-test-${crypto.randomUUID()}`);
    const scope = { organizationId: "org-1", documentId: "doc-1" };
    const writer = await LoroDocumentSession.open({
      persistence,
      scope,
      snapshotDebounceMs: 0,
    });
    writer.doc.getText("text").insert(0, "persisted");
    writer.doc.commit();
    await writer.flush();

    const restored = await LoroDocumentSession.open({ persistence, scope });
    expect(restored.doc.getText("text").toString()).toBe("persisted");
    await restored.dispose();
  });

  it("disposes active sessions before local document data is cleared", async () => {
    const persistence = new LoroDocumentPersistence(`riichi-test-${crypto.randomUUID()}`);
    const scope = { organizationId: "org-clear", documentId: "doc-clear" };
    const session = await LoroDocumentSession.open({ persistence, scope });
    session.doc.getText("text").insert(0, "private");
    session.doc.commit();
    await session.flush();

    await LoroDocumentSession.disposeAll();
    await persistence.clearAll();

    await expect(persistence.load(scope)).resolves.toEqual({
      snapshot: null,
      schemaVersion: null,
      updates: [],
    });
  });

  it("replays pending updates after an interrupted snapshot save", async () => {
    const persistence = new LoroDocumentPersistence(`riichi-test-${crypto.randomUUID()}`);
    const scope = { organizationId: "org-2", documentId: "doc-2" };
    const base = new LoroDocumentSession();
    const writer = new LoroDocumentSession(base.exportSnapshot());
    writer.doc.getText("text").insert(0, "pending");
    writer.doc.commit();
    const update = writer.exportUpdates();
    await persistence.saveSnapshot(scope, base.exportSnapshot());
    await persistence.appendUpdate(scope, "update-1", update);

    const restored = await LoroDocumentSession.open({ persistence, scope });
    expect(restored.doc.getText("text").toString()).toBe("pending");
    await restored.dispose();
  });

  it("clears an incompatible local snapshot before opening the server schema", async () => {
    const persistence = new LoroDocumentPersistence(`riichi-test-${crypto.randomUUID()}`);
    const scope = { organizationId: "org-schema", documentId: "doc-schema" };
    const stale = new LoroDocumentSession();
    stale.doc.getText("text").insert(0, "stale schema");
    stale.doc.commit();
    await persistence.saveSnapshot(scope, stale.exportSnapshot(), 2);

    const server = new LoroDocumentSession();
    server.doc.getText("text").insert(0, "server schema");
    server.doc.commit();
    const opened = await LoroDocumentSession.open({
      persistence,
      scope,
      serverSnapshot: server.exportSnapshot(),
      serverSchemaVersion: 1,
    });

    expect(opened.doc.getText("text").toString()).toBe("server schema");
    await expect(persistence.load(scope)).resolves.toMatchObject({ schemaVersion: 1 });
    await opened.dispose();
  });

  it("rejects a remote snapshot with an incompatible schema version", async () => {
    const session = new LoroDocumentSession(undefined, 1);
    const socket = new FakeWebSocket();
    let state = "disconnected";
    const connected = session.connectWebSocket(
      "ws://example.test/doc",
      () => socket as unknown as WebSocket,
      (nextState) => { state = nextState; },
    );
    socket.open();
    await connected;

    const remote = new LoroDocumentSession();
    remote.doc.getText("text").insert(0, "unsupported");
    remote.doc.commit();
    const payload = remote.exportSnapshot();
    const envelope = new TextEncoder().encode(JSON.stringify({ type: "snapshot", schema_version: 2 }));
    const frame = new Uint8Array(envelope.byteLength + 1 + payload.byteLength);
    frame.set(envelope);
    frame[envelope.byteLength] = 0x0a;
    frame.set(payload, envelope.byteLength + 1);
    socket.deliver(frame.buffer);
    await Promise.resolve();

    expect(session.doc.getText("text").toString()).toBe("");
    expect(state).toBe("error");
    await session.dispose();
  });

  it("purges the scoped local document when the server revokes access", async () => {
    const persistence = new LoroDocumentPersistence(`riichi-test-${crypto.randomUUID()}`);
    const scope = { organizationId: "org-revoked", documentId: "doc-revoked" };
    const session = await LoroDocumentSession.open({ persistence, scope });
    session.doc.getText("text").insert(0, "private");
    session.doc.commit();
    await session.flush();
    const socket = new FakeWebSocket();
    const connected = session.connectWebSocket(
      "ws://example.test/doc",
      () => socket as unknown as WebSocket,
    );
    socket.open();
    await connected;

    socket.deliver(JSON.stringify({
      type: "error",
      retryable: false,
      message: "document access was revoked",
    }));
    await vi.waitFor(async () => {
      await expect(persistence.load(scope)).resolves.toEqual({
        snapshot: null,
        schemaVersion: null,
        updates: [],
      });
    });
    await session.dispose();
  });

  it("sends local updates and applies remote WebSocket snapshots", async () => {
    const session = new LoroDocumentSession();
    const socket = new FakeWebSocket();
    const connected = session.connectWebSocket("ws://example.test/doc", () => socket as unknown as WebSocket);
    socket.open();
    await connected;
    expect(JSON.parse(socket.sent[0] as string)).toMatchObject({
      type: "hello",
      peer_id: expect.any(String),
      schema_version: 2,
    });

    session.doc.getText("text").insert(0, "local");
    session.doc.commit();
    expect(socket.sent[1]).toBeInstanceOf(Uint8Array);
    const outbound = socket.sent[1] as Uint8Array;
    const separator = outbound.indexOf(0x0a);
    expect(JSON.parse(new TextDecoder().decode(outbound.slice(0, separator)))).toMatchObject({
      type: "update",
    });

    const remote = new LoroDocumentSession();
    remote.doc.getText("text").insert(0, "remote");
    remote.doc.commit();
    const payload = remote.exportSnapshot();
    let binary = "";
    for (const byte of payload) binary += String.fromCharCode(byte);
    const envelope = new TextEncoder().encode(JSON.stringify({ type: "snapshot" }));
    const frame = new Uint8Array(envelope.byteLength + 1 + payload.byteLength);
    frame.set(envelope);
    frame[envelope.byteLength] = 0x0a;
    frame.set(payload, envelope.byteLength + 1);
    socket.deliver(frame.buffer);
    await Promise.resolve();
    expect(session.doc.getText("text").toString()).toContain("remote");
    await session.dispose();
  });

  it("can speak the v2 document protocol when explicitly selected", async () => {
    const session = new LoroDocumentSession(undefined, 2);
    const socket = new FakeWebSocket();
    const connected = session.connectWebSocket(
      "ws://example.test/doc",
      () => socket as unknown as WebSocket,
    );
    socket.open();
    await connected;

    expect(JSON.parse(socket.sent[0] as string)).toMatchObject({
      type: "hello",
      schema_version: 2,
    });
    await session.dispose();
  });

  it("requeues in-flight updates and reconnects after a dropped socket", async () => {
    vi.useFakeTimers();
    const session = new LoroDocumentSession();
    const sockets: FakeWebSocket[] = [];
    const connected = session.connectWebSocket("ws://example.test/doc", () => {
      const socket = new FakeWebSocket();
      sockets.push(socket);
      return socket as unknown as WebSocket;
    });
    sockets[0].open();
    await connected;
    session.doc.getText("text").insert(0, "offline");
    session.doc.commit();
    sockets[0].close();

    await vi.advanceTimersByTimeAsync(250);
    expect(sockets).toHaveLength(2);
    sockets[1].open();
    expect(sockets[1].sent).toHaveLength(2);
    expect(sockets[1].sent[1]).toBeInstanceOf(Uint8Array);
    await session.dispose();
  });

  it("purges local data when reconnect authorization is revoked while offline", async () => {
    const persistence = new LoroDocumentPersistence(`riichi-test-${crypto.randomUUID()}`);
    const scope = { organizationId: "org-offline-revoked", documentId: "doc-offline-revoked" };
    const session = await LoroDocumentSession.open({ persistence, scope });
    session.doc.getText("text").insert(0, "private");
    session.doc.commit();
    await session.flush();
    const sockets: FakeWebSocket[] = [];
    const connected = session.connectWebSocket(
      "ws://example.test/doc",
      () => {
        const socket = new FakeWebSocket();
        sockets.push(socket);
        return socket as unknown as WebSocket;
      },
      undefined,
      async () => "revoked",
    );
    sockets[0].open();
    await connected;
    sockets[0].close();

    await new Promise((resolve) => setTimeout(resolve, 300));
    await expect(persistence.load(scope)).resolves.toEqual({
      snapshot: null,
      schemaVersion: null,
      updates: [],
    });
    expect(sockets).toHaveLength(1);
    await session.dispose();
  });
});
