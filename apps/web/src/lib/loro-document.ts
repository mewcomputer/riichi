import { LoroDoc } from "loro-crdt";
import {
  LoroSyncPlugin,
  LoroUndoPlugin,
  type LoroDocType,
} from "loro-prosemirror";
import {
  LoroDocumentPersistence,
  type LoroSessionPersistenceOptions,
} from "./loro-persistence";

export type LoroSyncState = "disconnected" | "connecting" | "connected" | "reconnecting" | "error";
export type LoroSyncAuthorization = "allowed" | "revoked" | "unavailable";

export const LORO_DOCUMENT_SCHEMA_VERSION = 2;
export const LORO_DOCUMENT_SCHEMA_V2 = 2;

export class LoroDocumentSession {
  private static readonly activeSessions = new Set<LoroDocumentSession>();
  readonly doc: LoroDoc;
  private persistence:
    | { store: LoroDocumentPersistence; scope: LoroSessionPersistenceOptions["scope"]; debounceMs: number }
    | undefined;
  private persistenceQueue = Promise.resolve();
  private persistTimer: ReturnType<typeof setTimeout> | undefined;
  private stopPersistence: (() => void) | undefined;
  private stopLocalSync: (() => void) | undefined;
  private syncSocket: WebSocket | undefined;
  private syncQueue: Array<{ updateId: string; payload: Uint8Array }> = [];
  private syncInFlight = new Map<string, Uint8Array>();
  private syncOpen = false;
  private syncUrl: string | undefined;
  private syncWebSocketFactory: ((url: string) => WebSocket) | undefined;
  private syncAuthorizationCheck: (() => Promise<LoroSyncAuthorization>) | undefined;
  private reconnectTimer: ReturnType<typeof setTimeout> | undefined;
  private reconnectAttempt = 0;
  private syncStateListener: ((state: LoroSyncState) => void) | undefined;
  private syncStopped = false;
  private readonly schemaVersion: number;

  constructor(snapshot?: Uint8Array, schemaVersion = LORO_DOCUMENT_SCHEMA_VERSION) {
    this.doc = new LoroDoc();
    this.schemaVersion = schemaVersion;
    if (snapshot) this.doc.import(snapshot);
    LoroDocumentSession.activeSessions.add(this);
  }

  static async disposeAll(): Promise<void> {
    const sessions = [...LoroDocumentSession.activeSessions];
    await Promise.allSettled(sessions.map((session) => session.dispose()));
  }

  static async open(options: LoroSessionPersistenceOptions): Promise<LoroDocumentSession> {
    let stored = await options.persistence.load(options.scope);
    const expectedSchemaVersion =
      options.serverSchemaVersion ?? options.clientSchemaVersion ?? LORO_DOCUMENT_SCHEMA_VERSION;
    if (
      stored.snapshot &&
      (stored.schemaVersion ?? LORO_DOCUMENT_SCHEMA_VERSION) !== expectedSchemaVersion
    ) {
      if (!options.serverSnapshot) {
        throw new Error("stored document schema version is incompatible with the client");
      }
      await options.persistence.clear(options.scope);
      stored = { snapshot: null, schemaVersion: null, updates: [] };
    }
    const session = new LoroDocumentSession(
      stored.snapshot ?? options.serverSnapshot,
      expectedSchemaVersion,
    );
    if (stored.snapshot && options.serverSnapshot) session.doc.import(options.serverSnapshot);
    for (const update of stored.updates) {
      session.doc.import(update.payload);
      session.syncQueue.push({ updateId: update.id, payload: update.payload.slice() });
    }
    session.attachPersistence(options);
    if (!stored.snapshot && options.serverSnapshot) {
      await options.persistence.saveSnapshot(
        options.scope,
        options.serverSnapshot,
        expectedSchemaVersion,
      );
    }
    return session;
  }

  exportSnapshot(): Uint8Array {
    return this.doc.export({ mode: "snapshot" });
  }

  exportUpdates(): Uint8Array {
    return this.doc.export({ mode: "update" });
  }

  applyUpdate(update: Uint8Array): void {
    if (update.byteLength === 0) throw new Error("Loro update payload must not be empty");
    this.doc.import(update);
  }

  subscribeLocalUpdates(listener: (update: Uint8Array) => void): () => void {
    return this.doc.subscribeLocalUpdates(listener);
  }

  frontierCount(): number {
    return this.doc.frontiers().length;
  }

  flush(): Promise<void> {
    if (this.persistTimer !== undefined) {
      clearTimeout(this.persistTimer);
      this.persistTimer = undefined;
    }
    if (!this.persistence || this.syncQueue.length > 0 || this.syncInFlight.size > 0) {
      return Promise.resolve();
    }
      return this.enqueue(() => this.persistence!.store.saveSnapshot(
        this.persistence!.scope,
        this.exportSnapshot(),
        this.schemaVersion,
      ));
  }

  dispose(): Promise<void> {
    LoroDocumentSession.activeSessions.delete(this);
    this.syncStopped = true;
    if (this.reconnectTimer !== undefined) clearTimeout(this.reconnectTimer);
    this.reconnectTimer = undefined;
    this.stopPersistence?.();
    this.stopPersistence = undefined;
    this.stopLocalSync?.();
    this.stopLocalSync = undefined;
    this.syncSocket?.close();
    this.syncSocket = undefined;
    this.syncOpen = false;
    this.syncUrl = undefined;
    this.setSyncState("disconnected");
    return this.flush();
  }

  connectWebSocket(
    url: string,
    webSocketFactory: (url: string) => WebSocket = (target) => new WebSocket(target),
    onStateChange?: (state: LoroSyncState) => void,
    authorizationCheck?: () => Promise<LoroSyncAuthorization>,
  ): Promise<void> {
    if (this.syncSocket || this.syncUrl) throw new Error("Loro WebSocket sync is already connected");
    this.syncUrl = url;
    this.syncWebSocketFactory = webSocketFactory;
    this.syncStateListener = onStateChange;
    this.syncAuthorizationCheck = authorizationCheck;
    this.syncStopped = false;
    this.setSyncState("connecting");
    return this.openSyncSocket(true);
  }

  private openSyncSocket(initial: boolean): Promise<void> {
    if (!initial && this.syncAuthorizationCheck) {
      return this.checkAuthorizationBeforeReconnect();
    }
    const socket = this.syncWebSocketFactory!(this.syncUrl!);
    this.syncSocket = socket;
    socket.binaryType = "arraybuffer";
    if (!this.persistence) {
      this.stopLocalSync = this.doc.subscribeLocalUpdates((update) => {
        this.queueSyncUpdate(update);
      });
    }
    socket.onmessage = (event) => {
      void this.handleSyncMessage(event.data);
    };
    return new Promise((resolve, reject) => {
      let settled = false;
      socket.onopen = () => {
        this.syncOpen = true;
        this.reconnectAttempt = 0;
        this.setSyncState("connected");
        socket.send(JSON.stringify({
          type: "hello",
          peer_id: this.doc.peerIdStr,
          schema_version: this.schemaVersion,
        }));
        this.flushSyncQueue();
        settled = true;
        resolve();
      };
      socket.onerror = () => {
        this.setSyncState("error");
        if (initial && !settled) reject(new Error("Loro WebSocket sync failed to connect"));
      };
      socket.onclose = () => {
        this.syncOpen = false;
        for (const [updateId, payload] of this.syncInFlight) {
          this.syncQueue.unshift({ updateId, payload });
        }
        this.syncInFlight.clear();
        if (this.syncSocket === socket) this.syncSocket = undefined;
        if (initial && !settled) reject(new Error("Loro WebSocket sync closed before connecting"));
        if (!this.syncStopped) this.scheduleReconnect();
      };
    });
  }

  private async checkAuthorizationBeforeReconnect(): Promise<void> {
    const authorization = await this.syncAuthorizationCheck!();
    if (authorization === "revoked") {
      await this.purgeLocalDocumentState();
      this.setSyncState("error");
      return;
    }
    if (authorization === "unavailable") {
      this.scheduleReconnect();
      return;
    }
    await this.openSyncSocket(true);
  }

  private scheduleReconnect() {
    if (this.reconnectTimer !== undefined || !this.syncUrl || !this.syncWebSocketFactory) return;
    this.reconnectAttempt += 1;
    this.setSyncState("reconnecting");
    const delay = Math.min(10_000, 250 * 2 ** Math.min(this.reconnectAttempt - 1, 5));
    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = undefined;
      if (this.syncStopped) return;
      void this.openSyncSocket(false).catch(() => undefined);
    }, delay);
  }

  private setSyncState(state: LoroSyncState) {
    this.syncStateListener?.(state);
  }

  private attachPersistence(options: LoroSessionPersistenceOptions) {
    this.persistence = {
      store: options.persistence,
      scope: options.scope,
      debounceMs: options.snapshotDebounceMs ?? 500,
    };
    this.stopPersistence = this.doc.subscribeLocalUpdates((update) => {
      const updateId = crypto.randomUUID();
      this.enqueue(() => this.persistence!.store.appendUpdate(this.persistence!.scope, updateId, update));
      this.queueSyncUpdate(update, updateId);
      this.scheduleSnapshot();
    });
  }

  private queueSyncUpdate(update: Uint8Array, updateId = crypto.randomUUID()) {
    this.syncQueue.push({ updateId, payload: update.slice() });
    this.flushSyncQueue();
  }

  private flushSyncQueue() {
    if (!this.syncOpen || !this.syncSocket || this.syncInFlight.size > 0) return;
    const next = this.syncQueue.shift();
    if (!next) return;
    this.syncInFlight.set(next.updateId, next.payload);
    try {
      this.syncSocket.send(encodeBinaryUpdate(next.updateId, next.payload));
    } catch {
      this.syncInFlight.delete(next.updateId);
      this.syncQueue.unshift(next);
    }
  }

  private async handleSyncMessage(data: unknown) {
    let message: {
      type?: string;
      payload_base64?: string;
      update_id?: string;
      retryable?: boolean;
      message?: string;
      schema_version?: number;
    };
    let binaryPayload: Uint8Array | undefined;
    if (data instanceof ArrayBuffer) {
      const bytes = new Uint8Array(data);
      const separator = bytes.indexOf(0x0a);
      if (separator < 0) return;
      try {
        message = JSON.parse(new TextDecoder().decode(bytes.slice(0, separator))) as typeof message;
      } catch {
        return;
      }
      binaryPayload = bytes.slice(separator + 1);
    } else if (typeof data === "string") {
      try {
        message = JSON.parse(data) as typeof message;
      } catch {
        return;
      }
    } else {
      return;
    }
    if (message.type === "accepted" && message.update_id) {
      this.syncInFlight.delete(message.update_id);
      this.flushSyncQueue();
      if (this.syncQueue.length === 0 && this.syncInFlight.size === 0) this.scheduleSnapshot();
      return;
    }
    if (message.type === "error") {
      if (message.retryable === false) {
        this.setSyncState("error");
        if (message.message === "document access was revoked") {
          await this.purgeLocalDocumentState();
        }
        return;
      }
      this.syncSocket?.close();
      return;
    }
    if (message.type !== "snapshot" && message.type !== "update") return;
    if (
      message.type === "snapshot" &&
      message.schema_version !== undefined &&
      message.schema_version !== this.schemaVersion
    ) {
      this.setSyncState("error");
      return;
    }
    if (!binaryPayload && !message.payload_base64) {
      this.setSyncState("error");
      return;
    }
    const payload = binaryPayload ?? base64ToBytes(message.payload_base64!);
    this.applyUpdate(payload);
    if (message.type === "update" && message.update_id && this.persistence) {
      await this.enqueue(() => this.persistence!.store.appendUpdate(
        this.persistence!.scope,
        message.update_id!,
        payload,
      ));
    }
    this.scheduleSnapshot();
  }

  private scheduleSnapshot() {
    if (this.persistTimer !== undefined) clearTimeout(this.persistTimer);
    this.persistTimer = setTimeout(() => {
      this.persistTimer = undefined;
      void this.flush();
    }, this.persistence?.debounceMs ?? 500);
  }

  private async purgeLocalDocumentState(): Promise<void> {
    this.syncStopped = true;
    if (this.persistTimer !== undefined) clearTimeout(this.persistTimer);
    this.persistTimer = undefined;
    this.stopPersistence?.();
    this.stopPersistence = undefined;
    this.stopLocalSync?.();
    this.stopLocalSync = undefined;
    this.syncQueue = [];
    this.syncInFlight.clear();
    this.syncSocket?.close();
    this.syncSocket = undefined;
    this.syncOpen = false;
    this.syncUrl = undefined;
    const persistence = this.persistence;
    this.persistence = undefined;
    if (persistence) await persistence.store.clear(persistence.scope);
  }

  private enqueue(task: () => Promise<void>): Promise<void> {
    this.persistenceQueue = this.persistenceQueue.then(task);
    return this.persistenceQueue;
  }
}

function encodeBinaryUpdate(updateId: string, payload: Uint8Array): Uint8Array {
  const envelope = new TextEncoder().encode(JSON.stringify({
    type: "update",
    update_id: updateId,
    idempotency_key: updateId,
  }));
  const frame = new Uint8Array(envelope.byteLength + 1 + payload.byteLength);
  frame.set(envelope);
  frame[envelope.byteLength] = 0x0a;
  frame.set(payload, envelope.byteLength + 1);
  return frame;
}

function base64ToBytes(value: string): Uint8Array {
  const binary = atob(value);
  return Uint8Array.from(binary, (character) => character.charCodeAt(0));
}

export function createLoroEditorPlugins(session: LoroDocumentSession) {
  const doc = session.doc as unknown as LoroDocType;
  return [LoroSyncPlugin({ doc }), LoroUndoPlugin({ doc })];
}
