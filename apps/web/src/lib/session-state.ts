type CleanupCollection = {
  cleanup: () => Promise<void>;
};

const sessionCollections = new Set<CleanupCollection>();

export function registerSessionCollection<T extends CleanupCollection>(collection: T): T {
  sessionCollections.add(collection);
  return collection;
}

export async function clearSessionCollections(): Promise<void> {
  const collections = [...sessionCollections];
  sessionCollections.clear();
  await Promise.allSettled(collections.map((collection) => collection.cleanup()));
}
