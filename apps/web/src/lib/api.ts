import { LORO_DOCUMENT_SCHEMA_VERSION } from "./loro-document";

export type HumanMembership = {
  project_id: string;
  project_name: string;
  role: "owner" | "admin" | "member" | "viewer";
};

export type HumanMe = {
  account_id: string;
  email: string | null;
  display_name: string | null;
  avatar_url: string | null;
  memberships: HumanMembership[];
  teams: Array<{
    team_id: string;
    team_name: string;
    team_key: string;
    role: "owner" | "admin" | "member" | "viewer";
  }>;
};

export type NavigationResponse = components["schemas"]["NavigationResponse"];

export type HumanQueueIssue = {
  team_id: string;
  team_name: string;
  team_key: string;
  project_id: string;
  project_name: string;
  id: string;
  display_key: string;
  title: string;
  body: string;
  status: "triage" | "todo" | "in_progress" | "blocked" | "done" | "canceled";
  importance: "none" | "low" | "medium" | "high" | "urgent";
  agent_eligible: boolean;
  spec_complete: boolean;
  specification_changed_since_review: boolean;
  unresolved_blocker_count: number;
  active_hold_count: number;
  active_lease_id: string | null;
  lease_expires_at: string | null;
  created_at: string;
  updated_at: string;
  rank: number;
  dispatch_version: number;
  assignee_account_id: string | null;
  labels: string[];
};

export type IssueEdge = {
  id: string;
  source_issue_id: string;
  target_issue_id: string;
  edge_type: "blocks" | "related" | "discovered_from" | "duplicate_of";
  created_at: string;
};

export type DispatchHold = {
  id: string;
  issue_id: string;
  hold_type: "manual" | "needs_spec" | "awaiting_approval" | "scheduled" | "integration";
  reason: string;
  created_by: string | null;
  created_at: string;
  expires_at: string | null;
  released_at: string | null;
};

export type IssueRecord = HumanQueueIssue & {
  parent_issue_id: string | null;
  children: Array<{
    id: string;
    display_key: string;
    title: string;
    status: HumanQueueIssue["status"];
    importance: HumanQueueIssue["importance"];
  }>;
  projects: Array<{
    project_id: string;
    project_name: string;
    created_at: string;
  }>;
  version: number;
  completed_at: string | null;
  rank_scope: string;
  edges: IssueEdge[];
  holds: DispatchHold[];
  active_owner_session_id: string | null;
  active_owner_role_id: string | null;
  quarantined_attempt_count: number;
  collaborators: LeaseCollaborator[];
  approvals: ApprovalRequest[];
  comments: CommentRecord[];
};

export type CommentRecord = {
  id: string;
  author_id: string;
  role_id: string | null;
  session_id: string | null;
  body: string;
  content: Record<string, unknown> | null;
  created_at: string;
};

export type ActivityRecord = {
  id: string;
  kind: string;
  actor_id: string;
  body: string | null;
  metadata: Record<string, unknown>;
  created_at: string;
};

export type DocumentRecord = {
  id: string;
  organization_id: string;
  kind: "issue_description" | "team_page" | "project_page" | "standalone_page";
  title: string;
  parent_document_id: string | null;
  position: number;
  owner_team_id: string | null;
  owner_project_id: string | null;
  provisioning_state: "pending" | "ready" | "failed" | "deleted";
  created_by: string;
  created_at: string;
  updated_at: string;
  deleted_at: string | null;
  current_revision: number | null;
  plain_text: string | null;
  sanitized_html: string | null;
};

export function getIssueDescriptionDocument(projectId: string, issueId: string) {
  return getJson<DocumentRecord>(
    `/api/v1/projects/${encodeURIComponent(projectId)}/issues/${encodeURIComponent(issueId)}/description-document`,
  );
}

export type DocumentVersion = {
  document_id: string;
  revision: number;
  content: Record<string, unknown>;
  plain_text: string;
  sanitized_html: string;
  schema_version: number;
  created_by: string;
  created_at: string;
};

export type DocumentReference = {
  document_id: string;
  source_block_id: string;
  resource_kind: "issue" | "team" | "project" | "document";
  resource_id: string;
  reference_kind: "inline" | "backlink";
  created_at: string;
};

export type Attachment = {
  id: string;
  organization_id: string;
  state: "pending" | "ready" | "quarantined" | "deleted";
  storage_key: string;
  filename: string;
  media_type: string;
  byte_size: number;
  checksum: number[];
  uploaded_by: string;
  created_at: string;
  completed_at: string | null;
  deleted_at: string | null;
};

export type AttachmentUpload = {
  upload_id: string;
  attachment_id: string;
  expires_at: string;
  upload_url: string;
};

export type Notification = {
  id: string;
  recipient_account_id: string;
  kind: "comment" | "approval" | "assignment" | "invitation" | "takeover" | "lease";
  project_id: string | null;
  issue_id: string | null;
  actor_id: string | null;
  payload: Record<string, unknown>;
  created_at: string;
  read_at: string | null;
};

export type InboxResponse = {
  notifications: Notification[];
  unread_count: number;
};

type GeneratedInboxResponse = NonNullable<
  NonNullable<paths["/api/v1/inbox"]["get"]>["responses"][200]["content"]
>["application/json"];

export type LeaseCollaborator = {
  lease_id: string;
  session_id: string;
  capability: string;
  grant_mode: "auto" | "approval_required";
  granted_by: string | null;
  granted_at: string;
  expires_at: string | null;
  revoked_at: string | null;
};

export type RecoveryChecklist = {
  id: string;
  issue_id: string;
  old_lease_id: string;
  old_session_id: string;
  initiated_by: string;
  reason: string;
  state: "open" | "completed" | "canceled";
  actions: unknown;
  created_at: string;
  completed_at: string | null;
};

export type ApprovalOperation =
  | { type: "set_rank"; rank: number }
  | { type: "reopen_for_dispatch"; checklist_id: string }
  | { type: "complete_with_summary"; checklist_id: string; resolution_summary: string };

export type ApprovalRequest = {
  id: string;
  issue_id: string;
  requested_by: string;
  target_version: number;
  proposed_operation: ApprovalOperation;
  state: "pending" | "approved" | "rejected" | "superseded" | "expired";
  expires_at: string;
  decided_by: string | null;
  decided_at: string | null;
  created_at: string;
};

export type GlobalApprovalRequest = ApprovalRequest & {
  project_id: string;
  team_key: string;
  project_name: string;
  issue_title: string;
};

export type AgentRole = {
  id: string;
  project_id: string;
  team_id: string;
  display_name: string;
  owner_account_id: string | null;
  capabilities: string[];
  revoked_at: string | null;
  active_session_count: number;
};

export type AgentSession = {
  id: string;
  project_id: string;
  team_id: string;
  agent_role_id: string;
  state: "active" | "expired" | "revoked";
  max_lifetime_ends_at: string;
  heartbeat_at: string | null;
  last_action_at: string | null;
  revoked_at: string | null;
};

export type AgentRoster = {
  roles: AgentRole[];
  sessions: AgentSession[];
};

type HumanQueueResponse = {
  issues: HumanQueueIssue[];
};

export class ApiError extends Error {
  readonly status: number;
  readonly code?: string;

  constructor(status: number, message: string, code?: string) {
    super(message);
    this.name = "ApiError";
    this.status = status;
    this.code = code;
  }
}

const apiBaseUrl = import.meta.env.VITE_API_BASE_URL ?? "";
const contractClient = createClient<paths>({ baseUrl: apiBaseUrl });

async function getJson<T>(path: string): Promise<T> {
  const response = await fetch(`${apiBaseUrl}${path}`, {
    credentials: "include",
    headers: { Accept: "application/json" },
  });

  if (!response.ok) {
    let errorBody: { code?: string; message?: string } = {};
    try {
      errorBody = (await response.json()) as typeof errorBody;
    } catch {
      // Preserve the HTTP error when the server did not return JSON.
    }
    throw new ApiError(
      response.status,
      errorBody.message ?? `Request failed with status ${response.status}`,
      errorBody.code,
    );
  }

  return (await response.json()) as T;
}

async function sendJson<T>(path: string, method: "POST" | "PATCH" | "PUT", body: unknown): Promise<T> {
  return (await sendJsonWithHeaders<T>(path, method, body)).data;
}

async function sendJsonWithHeaders<T>(
  path: string,
  method: "POST" | "PATCH" | "PUT",
  body: unknown,
): Promise<{ data: T; response: Response }> {
  const response = await fetch(`${apiBaseUrl}${path}`, {
    method,
    credentials: "include",
    headers: { Accept: "application/json", "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });

  if (!response.ok) {
    let errorBody: { code?: string; message?: string } = {};
    try {
      errorBody = (await response.json()) as typeof errorBody;
    } catch {
      // Preserve the HTTP error when the server did not return JSON.
    }
    throw new ApiError(
      response.status,
      errorBody.message ?? `Request failed with status ${response.status}`,
      errorBody.code,
    );
  }

  return { data: (await response.json()) as T, response };
}

async function sendNoContent(path: string, method: "POST" | "PATCH" | "DELETE" = "POST", body?: unknown): Promise<void> {
  const response = await fetch(`${apiBaseUrl}${path}`, {
    method,
    credentials: "include",
    headers: { Accept: "application/json", ...(body === undefined ? {} : { "Content-Type": "application/json" }) },
    ...(body === undefined ? {} : { body: JSON.stringify(body) }),
  });
  if (!response.ok) {
    let errorBody: { code?: string; message?: string } = {};
    try {
      errorBody = (await response.json()) as typeof errorBody;
    } catch {
      // Preserve the HTTP error when the server did not return JSON.
    }
    throw new ApiError(
      response.status,
      errorBody.message ?? `Request failed with status ${response.status}`,
      errorBody.code,
    );
  }
}

export function getCurrentUser() {
  return getJson<HumanMe>("/api/v1/auth/me");
}

export async function getNavigation() {
  const { data, error, response } = await contractClient.GET("/api/v1/navigation", {
    credentials: "include",
  });
  if (!response.ok) {
    const errorBody = error as { code?: string; message?: string } | undefined;
    throw new ApiError(
      response.status,
      errorBody?.message ?? `Request failed with status ${response.status}`,
      errorBody?.code,
    );
  }
  return data;
}

export function getPendingApprovals() {
  return getJson<GlobalApprovalRequest[]>("/api/v1/approvals");
}

export function getInbox(options?: { unreadOnly?: boolean; limit?: number }) {
  const params = new URLSearchParams();
  if (options?.unreadOnly) params.set("unread_only", "true");
  if (options?.limit !== undefined) params.set("limit", String(options.limit));
  const query = params.toString();
  return getJson<GeneratedInboxResponse>(`/api/v1/inbox${query ? `?${query}` : ""}`);
}

export function markInboxNotificationRead(notificationId: string) {
  return sendNoContent(`/api/v1/inbox/${encodeURIComponent(notificationId)}/read`);
}

export function logout() {
  return sendNoContent("/auth/logout");
}

export async function uploadAvatar(file: File) {
  const body = new FormData();
  body.append("avatar", file);
  const response = await fetch(`${apiBaseUrl}/api/v1/auth/me/avatar`, {
    method: "PUT",
    credentials: "include",
    body,
  });
  if (!response.ok) throw new ApiError(response.status, "Could not update profile image");
}

export async function uploadOrganizationLogo(organizationId: string, file: File) {
  const body = new FormData();
  body.append("logo", file);
  const response = await fetch(`${apiBaseUrl}/api/v1/organizations/${encodeURIComponent(organizationId)}/logo`, {
    method: "PUT",
    credentials: "include",
    body,
  });
  if (!response.ok) throw new ApiError(response.status, "Could not update organization image");
}

export function deleteOrganizationLogo(organizationId: string) {
  return sendNoContent(`/api/v1/organizations/${encodeURIComponent(organizationId)}/logo`, "DELETE");
}

export function deleteAvatar() {
  return sendNoContent("/api/v1/auth/me/avatar", "DELETE");
}

export async function getProjectQueue(projectId: string) {
  const response = await getJson<HumanQueueResponse>(
    `/api/v1/projects/${encodeURIComponent(projectId)}/queue`,
  );
  return response.issues;
}

export async function getAllIssues(limit = 200) {
  const response = await getJson<HumanQueueResponse>(`/api/v1/issues?limit=${limit}`);
  return response.issues;
}

export async function getTeamIssues(teamId: string, limit = 200) {
  const response = await getJson<HumanQueueResponse>(
    `/api/v1/teams/${encodeURIComponent(teamId)}/issues?limit=${limit}`,
  );
  return response.issues;
}

export function getIssue(projectId: string, issueId: string) {
  return getJson<IssueRecord>(
    `/api/v1/projects/${encodeURIComponent(projectId)}/issues/${encodeURIComponent(issueId)}`,
  );
}

export function getGlobalIssue(issueId: string) {
  return getJson<IssueRecord>(`/api/v1/issues/${encodeURIComponent(issueId)}`);
}

export function createComment(projectId: string, issueId: string, content: Record<string, unknown>) {
  return sendJson<CommentRecord>(
    `/api/v1/projects/${encodeURIComponent(projectId)}/issues/${encodeURIComponent(issueId)}/comments`,
    "POST",
    { content },
  );
}

export function getIssueActivity(projectId: string, issueId: string, limit = 200) {
  return getJson<ActivityRecord[]>(
    `/api/v1/projects/${encodeURIComponent(projectId)}/issues/${encodeURIComponent(issueId)}/activity?limit=${limit}`,
  );
}

const emptyDocumentContent = { type: "doc", content: [] } as const;

export function getDocument(documentId: string) {
  return getJson<DocumentRecord>(`/api/v1/documents/${encodeURIComponent(documentId)}`);
}

export function updateDocumentMetadata(
  documentId: string,
  input: { title: string; parent_document_id?: string | null; position?: number },
) {
  return sendJson<DocumentRecord>(
    `/api/v1/documents/${encodeURIComponent(documentId)}`,
    "PATCH",
    input,
  );
}

export function deleteDocument(documentId: string) {
  return sendNoContent(`/api/v1/documents/${encodeURIComponent(documentId)}`, "DELETE");
}

export function getDocumentVersion(documentId: string, revision?: number) {
  const query = revision === undefined ? "" : `?revision=${encodeURIComponent(revision)}`;
  return getJson<DocumentVersion>(
    `/api/v1/documents/${encodeURIComponent(documentId)}/version${query}`,
  );
}

export type LoroFrontier = { peer_id: string; counter: number };

function encodeBase64(bytes: ArrayBuffer | Uint8Array) {
  const view = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes);
  let binary = "";
  for (const byte of view) binary += String.fromCharCode(byte);
  return btoa(binary);
}

export async function getDocumentLoroSnapshot(documentId: string, revision?: number) {
  const query = revision === undefined ? "" : `?revision=${encodeURIComponent(revision)}`;
  const response = await fetch(
    `${apiBaseUrl}/api/v1/documents/${encodeURIComponent(documentId)}/loro-snapshot${query}`,
    { credentials: "include" },
  );
  if (!response.ok) throw new Error(await response.text());
  return {
    revision: Number(response.headers.get("x-riichi-document-revision")),
    schema_version: Number(response.headers.get("x-riichi-document-schema-version")),
    frontiers: JSON.parse(response.headers.get("x-riichi-document-frontiers") ?? "[]") as Array<{
      peer: string;
      counter: number;
    }>,
    bytes: await response.arrayBuffer(),
  };
}

export function documentLoroWebSocketUrl(documentId: string): string {
  const url = new URL(
    `${apiBaseUrl}/api/v1/documents/${encodeURIComponent(documentId)}/loro-sync`,
    window.location.href,
  );
  url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
  return url.toString();
}

export function applyDocumentLoroUpdate(
  documentId: string,
  input: {
    schema_version?: number;
    update_id: string;
    idempotency_key?: string;
    previous_frontiers: LoroFrontier[];
    payload: ArrayBuffer | Uint8Array;
  },
) {
  return sendJson<{
    update_id: string;
    document_id: string;
    source: string;
    previous_frontiers: LoroFrontier[];
    resulting_frontiers: LoroFrontier[];
    accepted_at: string;
    replayed: boolean;
  }>(
    `/api/v1/documents/${encodeURIComponent(documentId)}/loro-updates`,
    "POST",
    {
      schema_version: input.schema_version ?? LORO_DOCUMENT_SCHEMA_VERSION,
      update_id: input.update_id,
      idempotency_key: input.idempotency_key,
      previous_frontiers: input.previous_frontiers,
      payload_base64: encodeBase64(input.payload),
    },
  );
}

export function createOrganizationDocument(
  organizationId: string,
  input: { title: string; parent_document_id?: string; position?: number; content?: Record<string, unknown> },
) {
  return sendJson<DocumentRecord>(
    `/api/v1/organizations/${encodeURIComponent(organizationId)}/documents`,
    "POST",
    { ...input, content: input.content ?? emptyDocumentContent },
  );
}

export function createTeamDocument(
  teamId: string,
  input: { title: string; parent_document_id?: string; position?: number; content?: Record<string, unknown> },
) {
  return sendJson<DocumentRecord>(
    `/api/v1/teams/${encodeURIComponent(teamId)}/documents`,
    "POST",
    { ...input, content: input.content ?? emptyDocumentContent },
  );
}

export function listOrganizationDocuments(organizationId: string, parentDocumentId?: string) {
  const query = parentDocumentId
    ? `?parent_document_id=${encodeURIComponent(parentDocumentId)}`
    : "";
  return getJson<DocumentRecord[]>(
    `/api/v1/organizations/${encodeURIComponent(organizationId)}/documents${query}`,
  );
}

export function listTeamDocuments(teamId: string, parentDocumentId?: string) {
  const query = parentDocumentId
    ? `?parent_document_id=${encodeURIComponent(parentDocumentId)}`
    : "";
  return getJson<DocumentRecord[]>(
    `/api/v1/teams/${encodeURIComponent(teamId)}/documents${query}`,
  );
}

export function listProjectDocuments(projectId: string, parentDocumentId?: string) {
  const query = parentDocumentId
    ? `?parent_document_id=${encodeURIComponent(parentDocumentId)}`
    : "";
  return getJson<DocumentRecord[]>(
    `/api/v1/projects/${encodeURIComponent(projectId)}/documents${query}`,
  );
}

export function createProjectDocument(
  projectId: string,
  input: { title: string; parent_document_id?: string; position?: number; content?: Record<string, unknown> },
) {
  return sendJson<DocumentRecord>(
    `/api/v1/projects/${encodeURIComponent(projectId)}/documents`,
    "POST",
    { ...input, content: input.content ?? emptyDocumentContent },
  );
}

export function updateDocumentContent(
  documentId: string,
  input: {
    expected_revision: number;
    content: Record<string, unknown>;
    references?: Array<{
      source_block_id: string;
      resource_kind: DocumentReference["resource_kind"];
      resource_id: string;
      reference_kind: DocumentReference["reference_kind"];
    }>;
  },
) {
  return sendJson<DocumentRecord>(
    `/api/v1/documents/${encodeURIComponent(documentId)}/version`,
    "PATCH",
    input,
  );
}

export function getDocumentReferences(documentId: string) {
  return getJson<DocumentReference[]>(
    `/api/v1/documents/${encodeURIComponent(documentId)}/references`,
  );
}

export function replaceDocumentReferences(
  documentId: string,
  references: Array<{
    source_block_id: string;
    resource_kind: DocumentReference["resource_kind"];
    resource_id: string;
    reference_kind: DocumentReference["reference_kind"];
  }>,
) {
  return sendJson<DocumentReference[]>(
    `/api/v1/documents/${encodeURIComponent(documentId)}/references`,
    "PUT",
    { references },
  );
}

export function getDocumentBacklinks(documentId: string) {
  return getJson<DocumentReference[]>(
    `/api/v1/documents/${encodeURIComponent(documentId)}/backlinks`,
  );
}

export function createAttachmentUpload(
  documentId: string,
  input: {
    filename: string;
    media_type: string;
    byte_size: number;
    checksum: string;
    source_block_id: string;
  },
) {
  return sendJson<AttachmentUpload>(
    `/api/v1/documents/${encodeURIComponent(documentId)}/attachments`,
    "POST",
    input,
  );
}

export async function putAttachmentUpload(uploadId: string, bytes: ArrayBuffer | Blob) {
  const response = await fetch(
    `${apiBaseUrl}/api/v1/attachment-uploads/${encodeURIComponent(uploadId)}`,
    { method: "PUT", credentials: "include", body: bytes },
  );
  if (!response.ok) throw new ApiError(response.status, "Could not upload attachment");
}

export function completeAttachmentUpload(uploadId: string) {
  return sendJson<Attachment>(
    `/api/v1/attachment-uploads/${encodeURIComponent(uploadId)}/complete`,
    "POST",
    undefined,
  );
}

export function attachmentUrl(attachmentId: string) {
  return `${apiBaseUrl}/api/v1/attachments/${encodeURIComponent(attachmentId)}`;
}

export function getAgentRoster(projectId: string) {
  return getJson<AgentRoster>(
    `/api/v1/projects/${encodeURIComponent(projectId)}/agents`,
  );
}

export function getTeamAgentRoster(teamId: string) {
  return getJson<AgentRoster>(`/api/v1/teams/${encodeURIComponent(teamId)}/agents`);
}

export function revokeAgentSession(projectId: string, sessionId: string) {
  return sendNoContent(
    `/api/v1/projects/${encodeURIComponent(projectId)}/agent-sessions/${encodeURIComponent(sessionId)}/revoke`,
  );
}

export function revokeAgentRole(projectId: string, roleId: string) {
  return sendNoContent(
    `/api/v1/projects/${encodeURIComponent(projectId)}/agent-roles/${encodeURIComponent(roleId)}/revoke`,
  );
}

export function createAgentRole(
  projectId: string,
  input: { display_name: string; owner_account_id?: string; capabilities: string[] },
) {
  return sendNoContent(
    `/api/v1/projects/${encodeURIComponent(projectId)}/agent-roles`,
    "POST",
    input,
  );
}

export type GithubImportResult = {
  repository: string;
  imported: number;
  pull_requests_skipped: number;
  issue_numbers: number[];
};

export type CreateProjectResult = { project_id: string };
export type CreateInviteResult = {
  invite_id: string;
  project_id: string;
  role: "viewer" | "member" | "admin";
  email_hint: string | null;
  token: string;
  expires_at: string;
};

export function createProject(name: string) {
  return sendJson<CreateProjectResult>("/api/v1/projects", "POST", { name });
}

export function updateTeamEmoji(teamId: string, emoji: string | null) {
  return sendNoContent(`/api/v1/teams/${encodeURIComponent(teamId)}`, "PATCH", { emoji });
}

export function createInvite(
  projectId: string,
  input: { role: CreateInviteResult["role"]; email_hint?: string; expires_in_seconds?: number },
) {
  return sendJson<CreateInviteResult>(
    `/api/v1/projects/${encodeURIComponent(projectId)}/invites`,
    "POST",
    input,
  );
}

export function importGithubIssues(
  projectId: string,
  input: { repository: string; max_issues?: number },
) {
  return sendJson<GithubImportResult>(
    `/api/v1/projects/${encodeURIComponent(projectId)}/integrations/github/import`,
    "POST",
    input,
  );
}

export function createIssue(
  projectId: string,
  input: {
    title: string;
    body: string;
    status?: HumanQueueIssue["status"];
    agent_eligible?: boolean;
    spec_complete?: boolean;
    rank?: number;
    labels?: string[];
    parent_issue_id?: string;
  },
) {
  return sendJson<IssueRecord>(
    `/api/v1/projects/${encodeURIComponent(projectId)}/issues`,
    "POST",
    input,
  );
}

export function updateIssue(
  projectId: string,
  issueId: string,
  input: {
    expected_version: number;
    title?: string;
    status?: HumanQueueIssue["status"];
    importance?: HumanQueueIssue["importance"];
    agent_eligible?: boolean;
    spec_complete?: boolean;
    rank?: number;
    labels?: string[];
  },
) {
  return sendJsonWithHeaders<IssueRecord>(
    `/api/v1/projects/${encodeURIComponent(projectId)}/issues/${encodeURIComponent(issueId)}`,
    "PATCH",
    input,
  ).then(({ data, response }) => Object.assign(data, {
    transactionId: Number(response.headers.get("x-riichi-transaction-id")),
  }));
}

export function createIssueEdge(
  projectId: string,
  input: { source_issue_id: string; target_issue_id: string; edge_type: IssueEdge["edge_type"] },
) {
  return sendJson<IssueEdge>(
    `/api/v1/projects/${encodeURIComponent(projectId)}/issues/${encodeURIComponent(input.source_issue_id)}/edges`,
    "POST",
    input,
  );
}

export function removeIssueEdge(projectId: string, edgeId: string) {
  return sendNoContent(
    `/api/v1/projects/${encodeURIComponent(projectId)}/edges/${encodeURIComponent(edgeId)}`,
  );
}

export function createIssueHold(
  projectId: string,
  issueId: string,
  input: { hold_type: DispatchHold["hold_type"]; reason: string; expires_in_seconds?: number },
) {
  return sendJson<IssueRecord>(
    `/api/v1/projects/${encodeURIComponent(projectId)}/issues/${encodeURIComponent(issueId)}/holds`,
    "POST",
    input,
  );
}

export function grantIssueCollaborator(
  projectId: string,
  issueId: string,
  input: { lease_id: string; session_id: string; capability: string; grant_mode: "auto" | "approval_required"; expires_in_seconds?: number },
) {
  return sendNoContent(
    `/api/v1/projects/${encodeURIComponent(projectId)}/issues/${encodeURIComponent(issueId)}/collaborators`,
    "POST",
    input,
  );
}

export function revokeIssueCollaborator(
  projectId: string,
  issueId: string,
  leaseId: string,
  sessionId: string,
  capability: string,
) {
  return sendNoContent(
    `/api/v1/projects/${encodeURIComponent(projectId)}/issues/${encodeURIComponent(issueId)}/collaborators/${encodeURIComponent(sessionId)}/${encodeURIComponent(capability)}/revoke?lease_id=${encodeURIComponent(leaseId)}`,
  );
}

export function releaseIssueHold(projectId: string, holdId: string) {
  return sendNoContent(
    `/api/v1/projects/${encodeURIComponent(projectId)}/holds/${encodeURIComponent(holdId)}/release`,
  );
}

export function getQuarantinedAttempts(projectId: string, issueId: string) {
  return getJson<QuarantinedAttempt[]>(
    `/api/v1/projects/${encodeURIComponent(projectId)}/issues/${encodeURIComponent(issueId)}/quarantined-attempts`,
  );
}

export type QuarantinedAttempt = {
  id: string;
  issue_id: string;
  session_id: string;
  role_id: string;
  lease_id: string;
  fencing_token: number;
  request_id: string;
  reason: string;
  payload: Record<string, unknown>;
  created_at: string;
};

export function takeoverIssue(projectId: string, issueId: string, reason: string) {
  return sendJson<RecoveryChecklist>(
    `/api/v1/projects/${encodeURIComponent(projectId)}/issues/${encodeURIComponent(issueId)}/takeover`,
    "POST",
    { reason },
  );
}

export function completeRecovery(
  projectId: string,
  checklistId: string,
  input: { expected_version: number; action: "release" | "complete"; resolution_summary?: string },
) {
  return sendJson<IssueRecord>(
    `/api/v1/projects/${encodeURIComponent(projectId)}/recovery/${encodeURIComponent(checklistId)}/complete`,
    "POST",
    input,
  );
}

export function createApprovalRequest(
  projectId: string,
  issueId: string,
  input: { target_version: number; proposed_operation: ApprovalOperation; expires_in_seconds?: number },
) {
  return sendJson<ApprovalRequest>(
    `/api/v1/projects/${encodeURIComponent(projectId)}/issues/${encodeURIComponent(issueId)}/approvals`,
    "POST",
    input,
  );
}

export function decideApprovalRequest(projectId: string, approvalId: string, approve: boolean) {
  return sendJson<ApprovalRequest>(
    `/api/v1/projects/${encodeURIComponent(projectId)}/approvals/${encodeURIComponent(approvalId)}/${approve ? "approve" : "reject"}`,
    "POST",
    {},
  );
}
import createClient from "openapi-fetch";
import type { components, paths } from "./generated/api";
