import { createCollection, type Collection } from "@tanstack/react-db";
import { electricCollectionOptions, isChangeMessage } from "@tanstack/electric-db-collection";

import {
  updateIssue,
  getIssue,
  type ActivityRecord,
  type AgentSession,
  type GlobalApprovalRequest,
  type HumanQueueIssue,
  type IssueRecord,
  type Notification,
  type DocumentRecord,
} from "./api";
import { authenticatedShapeUrl } from "./shape-url";
import { registerSessionCollection } from "./session-state";

export type IssueMetadataRecord = Pick<
  HumanQueueIssue,
  | "id"
  | "project_id"
  | "title"
  | "status"
  | "importance"
  | "agent_eligible"
  | "spec_complete"
  | "rank"
  | "labels"
  | "assignee_account_id"
> & { version: number; transaction_id: number };

export type IssueMetadataChanges = Partial<Pick<
  IssueMetadataRecord,
  "title" | "status" | "importance" | "agent_eligible" | "spec_complete" | "rank" | "labels" | "assignee_account_id"
>>;

export type IssueMetadataCollection = Collection<IssueMetadataRecord>;

export type HumanIssueSyncRecord = Omit<HumanQueueIssue, "id"> & {
  account_id: string;
  issue_id: string;
  transaction_id: number;
};

export function issueFromSyncRecord({ issue_id, ...record }: HumanIssueSyncRecord): HumanQueueIssue {
  return { ...record, id: issue_id };
}

export type HumanDocumentSyncRecord = Omit<DocumentRecord, "id"> & {
  account_id: string;
  document_id: string;
  transaction_id: number;
};

export function documentFromSyncRecord({ document_id, ...record }: HumanDocumentSyncRecord): DocumentRecord {
  return { ...record, id: document_id };
}

export type HumanAgentSyncRecord = {
  account_id: string;
  agent_role_id: string;
  project_id: string;
  team_id: string;
  display_name: string;
  owner_account_id: string | null;
  capabilities: string[];
  revoked_at: string | null;
  active_session_count: number;
  sessions: AgentSession[];
  transaction_id: number;
};

export type IssueActivitySyncRecord = ActivityRecord & {
  project_id: string;
  issue_id: string;
};

function mutationInput(changes: IssueMetadataChanges, expectedVersion: number) {
  return {
    expected_version: expectedVersion,
    ...(changes.title === undefined ? {} : { title: changes.title }),
    ...(changes.status === undefined ? {} : { status: changes.status }),
    ...(changes.importance === undefined ? {} : { importance: changes.importance }),
    ...(changes.agent_eligible === undefined ? {} : { agent_eligible: changes.agent_eligible }),
    ...(changes.spec_complete === undefined ? {} : { spec_complete: changes.spec_complete }),
    ...(changes.rank === undefined ? {} : { rank: changes.rank }),
    ...(changes.labels === undefined ? {} : { labels: changes.labels }),
    ...(changes.assignee_account_id === undefined ? {} : { assignee_account_id: changes.assignee_account_id }),
  };
}

export async function updateIssueMetadata(
  collection: IssueMetadataCollection | null,
  projectId: string,
  issueId: string,
  changes: IssueMetadataChanges,
): Promise<IssueRecord | null> {
  if (!collection?.get(issueId)) {
    const current = await getIssue(projectId, issueId);
    return updateIssue(projectId, issueId, {
      expected_version: current.version,
      ...changes,
    });
  }

  const transaction = collection.update(issueId, (draft) => {
    if (changes.title !== undefined) draft.title = changes.title;
    if (changes.status !== undefined) draft.status = changes.status;
    if (changes.importance !== undefined) draft.importance = changes.importance;
    if (changes.agent_eligible !== undefined) draft.agent_eligible = changes.agent_eligible;
    if (changes.spec_complete !== undefined) draft.spec_complete = changes.spec_complete;
    if (changes.rank !== undefined) draft.rank = changes.rank;
    if (changes.labels !== undefined) draft.labels = changes.labels;
    if (changes.assignee_account_id !== undefined) draft.assignee_account_id = changes.assignee_account_id;
  });
  await transaction.isPersisted.promise;
  return null;
}

/**
 * Creates the optional Electric-backed metadata collection.
 *
 * The browser always connects to Riichi's authenticated same-origin proxy.
 * TanStack Query remains the fallback read path while replication is disabled
 * or unavailable, and every mutation still goes through the named Riichi API
 * command so optimistic rollback and version checks remain authoritative.
 */
export function createIssueMetadataCollection(projectId: string): IssueMetadataCollection | null {
  const electricEnabled = import.meta.env.VITE_ELECTRIC_SYNC_ENABLED === "true";
  if (!electricEnabled) return null;

  return registerSessionCollection(createCollection(
    electricCollectionOptions<IssueMetadataRecord>({
      id: `riichi-issues-${projectId}`,
      getKey: (issue) => issue.id,
      shapeOptions: {
        url: authenticatedShapeUrl(`/api/v1/projects/${encodeURIComponent(projectId)}/sync/issues`),
      },
      onUpdate: async ({ transaction, collection }) => {
        for (const mutation of transaction.mutations) {
          const updated = await updateIssue(
            mutation.original.project_id,
            mutation.original.id,
            mutationInput(mutation.changes, mutation.original.version),
          );
          const transactionId = updated.transactionId;
          const acknowledged = await collection.utils.awaitMatch(
            (message) =>
              isChangeMessage(message) &&
              message.key === mutation.original.id &&
              message.value.version === updated.version &&
              message.value.transaction_id === transactionId,
          );
          if (!acknowledged) {
            throw new Error("issue metadata update was not acknowledged by Electric");
          }
        }
      },
    }),
  ));
}

export function createHumanIssueCollection() {
  if (import.meta.env.VITE_ELECTRIC_SYNC_ENABLED !== "true") return null;

  return registerSessionCollection(createCollection(
    electricCollectionOptions<HumanIssueSyncRecord>({
      id: "riichi-human-issues",
      getKey: (issue) => issue.issue_id,
      shapeOptions: {
        url: authenticatedShapeUrl("/api/v1/sync/issues"),
      },
    }),
  ));
}

export function createHumanDocumentCollection() {
  if (import.meta.env.VITE_ELECTRIC_SYNC_ENABLED !== "true") return null;

  return registerSessionCollection(createCollection(
    electricCollectionOptions<HumanDocumentSyncRecord>({
      id: "riichi-human-documents",
      getKey: (document) => document.document_id,
      shapeOptions: {
        url: authenticatedShapeUrl("/api/v1/sync/documents"),
      },
    }),
  ));
}

export function createHumanAgentCollection(teamId: string) {
  if (import.meta.env.VITE_ELECTRIC_SYNC_ENABLED !== "true") return null;

  return registerSessionCollection(createCollection(
    electricCollectionOptions<HumanAgentSyncRecord>({
      id: `riichi-human-agents-${teamId}`,
      getKey: (role) => role.agent_role_id,
      shapeOptions: {
        url: authenticatedShapeUrl(`/api/v1/teams/${encodeURIComponent(teamId)}/sync/agents`),
      },
    }),
  ));
}

export function createIssueActivityCollection(projectId: string, issueId: string) {
  if (import.meta.env.VITE_ELECTRIC_SYNC_ENABLED !== "true") return null;

  return registerSessionCollection(createCollection(
    electricCollectionOptions<IssueActivitySyncRecord>({
      id: `riichi-issue-activity-${projectId}-${issueId}`,
      getKey: (activity) => activity.id,
      shapeOptions: {
        url: authenticatedShapeUrl(`/api/v1/projects/${encodeURIComponent(projectId)}/sync/issues/${encodeURIComponent(issueId)}/activity`),
      },
    }),
  ));
}

export function createNotificationCollection() {
  if (import.meta.env.VITE_ELECTRIC_SYNC_ENABLED !== "true") return null;

  return registerSessionCollection(createCollection(
    electricCollectionOptions<Notification>({
      id: "riichi-notifications",
      getKey: (notification) => notification.id,
      shapeOptions: {
        url: authenticatedShapeUrl("/api/v1/sync/inbox"),
      },
    }),
  ));
}

export function createApprovalCollection() {
  if (import.meta.env.VITE_ELECTRIC_SYNC_ENABLED !== "true") return null;

  return registerSessionCollection(createCollection(
    electricCollectionOptions<GlobalApprovalRequest>({
      id: "riichi-approvals",
      getKey: (approval) => `${approval.project_id}:${approval.id}`,
      shapeOptions: {
        url: authenticatedShapeUrl("/api/v1/sync/approvals"),
      },
    }),
  ));
}
