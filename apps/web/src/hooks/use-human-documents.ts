import { useMemo } from "react";
import { useLiveQuery } from "@tanstack/react-db";

import { createHumanDocumentCollection, documentFromSyncRecord } from "@/lib/metadata-sync";

export function useHumanDocuments() {
  const collection = useMemo(() => createHumanDocumentCollection(), []);
  const rows = useLiveQuery(() => collection, [collection]).data;
  return rows?.map(documentFromSyncRecord);
}
