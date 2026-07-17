const DATABASE_VERSION = 2;
const SNAPSHOTS_STORE = "snapshots";
const UPDATES_STORE = "updates";
export const LORO_PERSISTENCE_DATABASE = "riichi-loro";

export type LoroDocumentScope = {
  organizationId: string;
  documentId: string;
};

type SnapshotRecord = {
  key: string;
  snapshot: ArrayBuffer;
  schemaVersion?: number;
  updatedAt: number;
};

type UpdateRecord = {
  id: string;
  documentKey: string;
  payload: ArrayBuffer;
  createdAt: number;
};

export type StoredLoroDocument = {
  snapshot: Uint8Array | null;
  schemaVersion: number | null;
  updates: Array<{ id: string; payload: Uint8Array }>;
};

function scopeKey(scope: LoroDocumentScope) {
  return `${scope.organizationId}\u0000${scope.documentId}`;
}

function copyBytes(bytes: Uint8Array): ArrayBuffer {
  return bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) as ArrayBuffer;
}

function requestResult<T>(request: IDBRequest<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error("IndexedDB request failed"));
  });
}

function transactionComplete(transaction: IDBTransaction): Promise<void> {
  return new Promise((resolve, reject) => {
    transaction.oncomplete = () => resolve();
    transaction.onerror = () => reject(transaction.error ?? new Error("IndexedDB transaction failed"));
    transaction.onabort = () => reject(transaction.error ?? new Error("IndexedDB transaction aborted"));
  });
}

function openDatabase(name: string): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(name, DATABASE_VERSION);
    request.onupgradeneeded = () => {
      const database = request.result;
      if (!database.objectStoreNames.contains(SNAPSHOTS_STORE)) {
        database.createObjectStore(SNAPSHOTS_STORE, { keyPath: "key" });
      }
      if (!database.objectStoreNames.contains(UPDATES_STORE)) {
        const updates = database.createObjectStore(UPDATES_STORE, { keyPath: "id" });
        updates.createIndex("documentKey", "documentKey", { unique: false });
      }
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error("Could not open IndexedDB"));
  });
}

export class LoroDocumentPersistence {
  private readonly databasePromise: Promise<IDBDatabase>;

  constructor(private readonly databaseName = LORO_PERSISTENCE_DATABASE) {
    this.databasePromise = openDatabase(databaseName);
  }

  async load(scope: LoroDocumentScope): Promise<StoredLoroDocument> {
    const database = await this.databasePromise;
    const transaction = database.transaction([SNAPSHOTS_STORE, UPDATES_STORE], "readonly");
    const snapshotRequest = transaction.objectStore(SNAPSHOTS_STORE).get(scopeKey(scope));
    const updatesRequest = transaction
      .objectStore(UPDATES_STORE)
      .index("documentKey")
      .getAll(IDBKeyRange.only(scopeKey(scope)));
    const [snapshot, updates] = await Promise.all([
      requestResult<SnapshotRecord | undefined>(snapshotRequest),
      requestResult<UpdateRecord[]>(updatesRequest),
    ]);
    updates.sort((left, right) => left.createdAt - right.createdAt || left.id.localeCompare(right.id));
    return {
      snapshot: snapshot ? new Uint8Array(snapshot.snapshot) : null,
      schemaVersion: snapshot?.schemaVersion ?? null,
      updates: updates.map((update) => ({
        id: update.id.slice(update.id.lastIndexOf("\u0000") + 1),
        payload: new Uint8Array(update.payload),
      })),
    };
  }

  async appendUpdate(scope: LoroDocumentScope, updateId: string, payload: Uint8Array): Promise<void> {
    if (payload.byteLength === 0) throw new Error("Loro update payload must not be empty");
    const database = await this.databasePromise;
    const transaction = database.transaction(UPDATES_STORE, "readwrite");
    transaction.objectStore(UPDATES_STORE).put({
      id: `${scopeKey(scope)}\u0000${updateId}`,
      documentKey: scopeKey(scope),
      payload: copyBytes(payload),
      createdAt: Date.now(),
    } satisfies UpdateRecord);
    await transactionComplete(transaction);
  }

  async saveSnapshot(
    scope: LoroDocumentScope,
    snapshot: Uint8Array,
    schemaVersion = 2,
  ): Promise<void> {
    if (snapshot.byteLength === 0) throw new Error("Loro snapshot must not be empty");
    const database = await this.databasePromise;
    const transaction = database.transaction([SNAPSHOTS_STORE, UPDATES_STORE], "readwrite");
    const key = scopeKey(scope);
    transaction.objectStore(SNAPSHOTS_STORE).put({
      key,
      snapshot: copyBytes(snapshot),
      schemaVersion,
      updatedAt: Date.now(),
    } satisfies SnapshotRecord);
    const updates = transaction.objectStore(UPDATES_STORE);
    const cursorRequest = updates.index("documentKey").openCursor(IDBKeyRange.only(key));
    cursorRequest.onsuccess = () => {
      const cursor = cursorRequest.result;
      if (!cursor) return;
      cursor.delete();
      cursor.continue();
    };
    await transactionComplete(transaction);
  }

  async clear(scope: LoroDocumentScope): Promise<void> {
    const database = await this.databasePromise;
    const transaction = database.transaction([SNAPSHOTS_STORE, UPDATES_STORE], "readwrite");
    const key = scopeKey(scope);
    transaction.objectStore(SNAPSHOTS_STORE).delete(key);
    const updates = transaction.objectStore(UPDATES_STORE);
    const cursorRequest = updates.index("documentKey").openCursor(IDBKeyRange.only(key));
    cursorRequest.onsuccess = () => {
      const cursor = cursorRequest.result;
      if (!cursor) return;
      cursor.delete();
      cursor.continue();
    };
    await transactionComplete(transaction);
  }

  async clearAll(): Promise<void> {
    const database = await this.databasePromise;
    const transaction = database.transaction([SNAPSHOTS_STORE, UPDATES_STORE], "readwrite");
    transaction.objectStore(SNAPSHOTS_STORE).clear();
    transaction.objectStore(UPDATES_STORE).clear();
    await transactionComplete(transaction);
  }
}

export async function clearLoroDocumentPersistence(
  databaseName = LORO_PERSISTENCE_DATABASE,
): Promise<void> {
  await new LoroDocumentPersistence(databaseName).clearAll();
}

export type LoroSessionPersistenceOptions = {
  persistence: LoroDocumentPersistence;
  scope: LoroDocumentScope;
  serverSnapshot?: Uint8Array;
  serverSchemaVersion?: number;
  clientSchemaVersion?: number;
  snapshotDebounceMs?: number;
};
