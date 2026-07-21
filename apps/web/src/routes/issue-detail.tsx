import { useEffect, useMemo, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useLiveQuery } from "@tanstack/react-db";
import { Link, useNavigate, useParams } from "@tanstack/react-router";
import { ArrowLeft, CircleDot, LoaderCircle } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  ApiError,
  completeRecovery,
  createComment,
  createIssue,
  createIssueEdge,
  createIssueHold,
  createApprovalRequest,
  decideApprovalRequest,
  getAgentRoster,
  getCurrentUser,
  getGlobalIssue,
  getIssueDescriptionDocument,
  getDocumentVersion,
  getDocument,
  getDocumentLoroSnapshot,
  documentLoroWebSocketUrl,
  getQuarantinedAttempts,
  removeIssueEdge,
  releaseIssueHold,
  grantIssueCollaborator,
  revokeIssueCollaborator,
  takeoverIssue,
  updateIssue,
  updateDocumentMetadata,
  type ApprovalRequest,
  type IssueRecord,
  type RecoveryChecklist,
} from "@/lib/api";
import { ProjectHeader } from "@/components/project/project-header";
import { ProjectShell } from "@/components/project/project-shell";
import { ProjectSidebar } from "@/components/project/project-sidebar";
import { useAppLogout } from "../hooks/use-app-logout";
import { useActiveProject } from "../hooks/use-active-project";
import { useNavigation } from "../hooks/use-navigation";
import { useIssueActivity } from "../hooks/use-issue-activity";
import { useAllIssues } from "../hooks/use-all-issues";
import { useHumanAgentRoster } from "../hooks/use-human-agent-roster";
import { organizationSlug as toOrganizationSlug } from "../lib/organization-slug";
import { formatRelativeTime } from "../lib/utils";
import { RichTextTitleEditor, RichTextBodyEditor } from "@/components/issues/rich-text-issue-editor";
import { IssueStatusMenu } from "@/components/issues/issue-status-menu";
import { IssueImportanceMenu } from "@/components/issues/issue-importance-menu";
import { LazyIssueCreateDialog } from "@/components/issues/lazy-issue-create-dialog";
import { IssueActivityTimeline } from "@/components/issues/issue-activity-timeline";
import { DocumentEditor } from "@/components/documents/document-editor";
import { LoroDocumentPersistence } from "@/lib/loro-persistence";
import { LoroDocumentSession, type LoroSyncState } from "@/lib/loro-document";
import {
  createIssueActivityCollection,
  createIssueMetadataCollection,
  updateIssueMetadata,
  type IssueMetadataCollection,
} from "@/lib/metadata-sync";

type ActionFeedback = {
  state: "pending" | "confirmed" | "rejected";
  message: string;
};

function IssueEditor({
  issue,
  projectId,
  organizationSlug,
  organizationId,
  teamKey,
  canApprove,
  canComment,
  metadataCollection,
}: {
  issue: IssueRecord;
  projectId: string;
  organizationSlug: string;
  organizationId: string;
  teamKey: string;
  canApprove: boolean;
  canComment: boolean;
  metadataCollection: IssueMetadataCollection | null;
}) {
  const queryClient = useQueryClient();
  const descriptionDocumentQuery = useQuery({
    queryKey: ["issue-description-document", projectId, issue.id],
    queryFn: () => getIssueDescriptionDocument(projectId, issue.id),
  });
  const descriptionVersionQuery = useQuery({
    queryKey: ["issue-description-document", issue.id, "version"],
    queryFn: () => getDocumentVersion(descriptionDocumentQuery.data!.id),
    enabled: Boolean(descriptionDocumentQuery.data),
  });
  const [title, setTitle] = useState(issue.title);
  const [body, setBody] = useState(issue.body);
  const [descriptionContent, setDescriptionContent] = useState<Record<string, unknown>>({ type: "doc", content: [] });
  const [descriptionSession, setDescriptionSession] = useState<LoroDocumentSession | null>(null);
  const [descriptionSyncState, setDescriptionSyncState] = useState<LoroSyncState>("disconnected");
  const [descriptionSchemaVersion, setDescriptionSchemaVersion] = useState(1);
  const [importance, setImportance] = useState(issue.importance);
  const [specComplete, setSpecComplete] = useState(issue.spec_complete);
  const [syncConflict, setSyncConflict] = useState<IssueRecord | null>(null);
  const [takeoverReason, setTakeoverReason] = useState("");
  const [checklist, setChecklist] = useState<RecoveryChecklist | null>(null);
  const [approval, setApproval] = useState<ApprovalRequest | null>(null);
  const [proposedRank, setProposedRank] = useState(String(issue.rank));
  const [holdReason, setHoldReason] = useState("");
  const [blockerIssueId, setBlockerIssueId] = useState("");
  const [collaboratorSessionId, setCollaboratorSessionId] = useState("");
  const [collaboratorCapability, setCollaboratorCapability] = useState("comment");
  const [collaboratorMode, setCollaboratorMode] = useState<"auto" | "approval_required">("auto");
  const [subissueDialogOpen, setSubissueDialogOpen] = useState(false);
  const [recoveryFeedback, setRecoveryFeedback] = useState<ActionFeedback | null>(null);
  const [approvalFeedback, setApprovalFeedback] = useState<ActionFeedback | null>(null);
  const [relativeTimeNow, setRelativeTimeNow] = useState(() => new Date());
  const descriptionPersistence = useRef<LoroDocumentPersistence | null>(null);
  const initializedDescriptionId = useRef<string | null>(null);
  const allIssuesQuery = useAllIssues();
  const activityCollection = useMemo(
    () => createIssueActivityCollection(projectId, issue.id),
    [issue.id, projectId],
  );
  const activitySyncQuery = useLiveQuery(() => activityCollection, [activityCollection]);
  const rosterQuery = useQuery({
    queryKey: ["agents", projectId],
    queryFn: () => getAgentRoster(projectId),
    enabled: canApprove && Boolean(issue.active_lease_id),
  });
  const replicatedRoster = useHumanAgentRoster(issue.team_id);
  const projectRoster = replicatedRoster
    ? {
        roles: replicatedRoster.roles.filter((role) => role.project_id === projectId),
        sessions: replicatedRoster.sessions.filter((session) => session.project_id === projectId),
      }
    : rosterQuery.data;
  const quarantineQuery = useQuery({
    queryKey: ["quarantined-attempts", projectId, issue.id],
    queryFn: () => getQuarantinedAttempts(projectId, issue.id),
    enabled: canApprove && issue.quarantined_attempt_count > 0,
  });
  const activityQuery = useIssueActivity(projectId, issue.id);
  useEffect(() => {
    const documentId = descriptionDocumentQuery.data?.id;
    const version = descriptionVersionQuery.data;
    if (!documentId || !version || initializedDescriptionId.current === documentId) return;
    initializedDescriptionId.current = documentId;
    setDescriptionContent(version.content);
  }, [descriptionDocumentQuery.data?.id, descriptionVersionQuery.data]);
  useEffect(() => {
    const documentId = descriptionDocumentQuery.data?.id;
    if (!documentId) return;
    let disposed = false;
    const persistence = descriptionPersistence.current ?? new LoroDocumentPersistence();
    descriptionPersistence.current = persistence;
    let session: LoroDocumentSession | undefined;
    void (async () => {
      try {
        const snapshot = await getDocumentLoroSnapshot(documentId);
        setDescriptionSchemaVersion(snapshot.schema_version);
        const opened = await LoroDocumentSession.open({
          persistence,
          scope: { organizationId, documentId },
          serverSnapshot: new Uint8Array(snapshot.bytes),
          serverSchemaVersion: snapshot.schema_version,
        });
        session = opened;
        if (disposed) {
          await opened.dispose();
          return;
        }
        setDescriptionSession(opened);
        setDescriptionSyncState("connecting");
        try {
          await opened.connectWebSocket(documentLoroWebSocketUrl(documentId), undefined, setDescriptionSyncState, async () => {
            try {
              await getDocument(documentId);
              return "allowed";
            } catch (error) {
              return error instanceof ApiError && [401, 403, 404].includes(error.status)
                ? "revoked"
                : "unavailable";
            }
          });
        } catch {
          if (!disposed) setDescriptionSyncState("error");
        }
      } catch {
        if (!disposed) setDescriptionSyncState("error");
      }
    })();
    return () => {
      disposed = true;
      setDescriptionSession((current) => current === session ? null : current);
      void session?.dispose();
    };
  }, [descriptionDocumentQuery.data?.id, organizationId]);
  useEffect(() => {
    const interval = window.setInterval(() => setRelativeTimeNow(new Date()), 10_000);
    return () => window.clearInterval(interval);
  }, []);
  const issueMutationChain = useRef(Promise.resolve());
  const mutation = useMutation({
    mutationFn: (input: { title?: string; status?: IssueRecord["status"]; importance?: IssueRecord["importance"]; spec_complete?: boolean }) => {
      const request = issueMutationChain.current.then(async () => {
        return updateIssueMetadata(metadataCollection, projectId, issue.id, input);
      });
      issueMutationChain.current = request.then(() => undefined, () => undefined);
      return request;
    },
    onSuccess: (updated, input) => {
      setSyncConflict(null);
      if (updated) {
        setImportance(updated.importance);
        setSpecComplete(updated.spec_complete);
        queryClient.setQueryData(["issue", projectId, issue.id], updated);
        queryClient.setQueryData(["issue", "global", issue.id], updated);
      } else {
        if (input.importance !== undefined) setImportance(input.importance);
        if (input.spec_complete !== undefined) setSpecComplete(input.spec_complete);
      }
      if (input.title && descriptionDocumentQuery.data && input.title !== descriptionDocumentQuery.data.title) {
        void updateDocumentMetadata(descriptionDocumentQuery.data.id, {
          title: input.title,
          parent_document_id: descriptionDocumentQuery.data.parent_document_id,
          position: descriptionDocumentQuery.data.position,
        }).then((document) => {
          queryClient.setQueryData(["issue-description-document", projectId, issue.id], document);
        }).catch(() => undefined);
      }
      void queryClient.invalidateQueries({ queryKey: ["issue", "global", issue.id] });
      void queryClient.invalidateQueries({ queryKey: ["project", projectId, "queue"] });
    },
    onError: (error, input) => {
      const current = queryClient.getQueryData<IssueRecord>(["issue", projectId, issue.id]) ?? issue;
      if (input.importance !== undefined) setImportance(current.importance);
      if (input.spec_complete !== undefined) setSpecComplete(current.spec_complete);
      if (error instanceof ApiError && error.status === 409) {
        void getGlobalIssue(issue.id).then((serverIssue) => {
          queryClient.setQueryData(["issue", "global", issue.id], serverIssue);
          queryClient.setQueryData(["issue", projectId, issue.id], serverIssue);
          setSyncConflict(serverIssue);
        }).catch(() => undefined);
      }
    },
  });
  const hasDraftChanges = title.trim() !== issue.title;
  useEffect(() => {
    if (!hasDraftChanges || mutation.isPending) return;
    const timeout = window.setTimeout(() => {
      mutation.mutate({ title: title.trim() });
    }, 700);
    return () => window.clearTimeout(timeout);
  }, [hasDraftChanges, mutation.isPending, mutation.mutate, title]);
  const refresh = () => {
    void queryClient.invalidateQueries({ queryKey: ["issue", projectId, issue.id] });
    void queryClient.invalidateQueries({ queryKey: ["issue", "global", issue.id] });
    void queryClient.invalidateQueries({ queryKey: ["project", projectId, "queue"] });
  };
  const takeoverMutation = useMutation({
    mutationFn: () => takeoverIssue(projectId, issue.id, takeoverReason.trim()),
    onMutate: () => setRecoveryFeedback({ state: "pending", message: "Creating a recovery checklist…" }),
    onSuccess: (created) => {
      setChecklist(created);
      setTakeoverReason("");
      setRecoveryFeedback({ state: "confirmed", message: "Recovery checklist created and acknowledged by the server." });
      refresh();
    },
    onError: (error) => setRecoveryFeedback({ state: "rejected", message: error instanceof Error ? error.message : "Recovery checklist could not be created." }),
  });
  const recoveryMutation = useMutation({
    mutationFn: (action: "release" | "complete") => {
      if (!checklist) throw new Error("No recovery checklist is open.");
      return completeRecovery(projectId, checklist.id, {
        expected_version: issue.version,
        action,
        ...(action === "complete" ? { resolution_summary: "Recovered by a human operator." } : {}),
      });
    },
    onMutate: (action) => setRecoveryFeedback({ state: "pending", message: `${action === "release" ? "Reopening for dispatch" : "Completing recovery"} against issue version ${issue.version}…` }),
    onSuccess: (_updated, action) => {
      setChecklist(null);
      setRecoveryFeedback({ state: "confirmed", message: action === "release" ? "Issue reopened for dispatch. Server state is confirmed." : "Recovery completed. Server state is confirmed." });
      refresh();
    },
    onError: (error) => setRecoveryFeedback({ state: "rejected", message: error instanceof Error ? error.message : "Recovery action was rejected." }),
  });
  const approvalMutation = useMutation({
    mutationFn: () => createApprovalRequest(projectId, issue.id, {
      target_version: issue.version,
      proposed_operation: { type: "set_rank", rank: Number(proposedRank) },
    }),
    onMutate: () => setApprovalFeedback({ state: "pending", message: `Submitting rank ${proposedRank} against issue version ${issue.version}…` }),
    onSuccess: (created) => {
      setApproval(created);
      setApprovalFeedback({ state: "confirmed", message: `Approval request is pending against issue version ${created.target_version}.` });
    },
    onError: (error) => setApprovalFeedback({ state: "rejected", message: error instanceof Error ? error.message : "Approval request was rejected." }),
  });
  const decisionMutation = useMutation({
    mutationFn: (approve: boolean) => {
      if (!approval) throw new Error("No approval request is open.");
      return decideApprovalRequest(projectId, approval.id, approve);
    },
    onMutate: (approve) => setApprovalFeedback({ state: "pending", message: `${approve ? "Approving" : "Rejecting"} the request against issue version ${approval?.target_version ?? issue.version}…` }),
    onSuccess: (decided, approve) => {
      setApproval(decided);
      setApprovalFeedback({ state: "confirmed", message: `Approval request ${approve ? "approved" : "rejected"}. Server state is confirmed.` });
      refresh();
    },
    onError: (error) => setApprovalFeedback({ state: "rejected", message: error instanceof Error ? error.message : "Approval decision was rejected." }),
  });
  const holdMutation = useMutation({
    mutationFn: () => createIssueHold(projectId, issue.id, { hold_type: "manual", reason: holdReason.trim() }),
    onSuccess: () => { setHoldReason(""); refresh(); },
  });
  const releaseHoldMutation = useMutation({
    mutationFn: (holdId: string) => releaseIssueHold(projectId, holdId),
    onSuccess: refresh,
  });
  const edgeMutation = useMutation({
    mutationFn: () => createIssueEdge(projectId, {
      source_issue_id: blockerIssueId.trim(),
      target_issue_id: issue.id,
      edge_type: "blocks",
    }),
    onSuccess: () => { setBlockerIssueId(""); refresh(); },
  });
  const removeEdgeMutation = useMutation({
    mutationFn: (edgeId: string) => removeIssueEdge(projectId, edgeId),
    onSuccess: refresh,
  });
  const grantCollaboratorMutation = useMutation({
    mutationFn: () => grantIssueCollaborator(projectId, issue.id, {
      lease_id: issue.active_lease_id!,
      session_id: collaboratorSessionId,
      capability: collaboratorCapability,
      grant_mode: collaboratorMode,
    }),
    onSuccess: refresh,
  });
  const revokeCollaboratorMutation = useMutation({
    mutationFn: (input: { sessionId: string; capability: string }) => revokeIssueCollaborator(
      projectId,
      issue.id,
      issue.active_lease_id!,
      input.sessionId,
      input.capability,
    ),
    onSuccess: refresh,
  });
  const commentMutation = useMutation({
    mutationFn: (content: Record<string, unknown>) => createComment(projectId, issue.id, content),
    onSuccess: refresh,
  });
  const syncedActivity = activitySyncQuery.data ?? [];
  const syncedComments = syncedActivity
    .filter((activity) => activity.kind === "comment")
    .map((activity) => ({
      id: activity.id,
      author_id: activity.actor_id,
      role_id: null,
      session_id: null,
      body: activity.body ?? "",
      content: activity.metadata,
      created_at: activity.created_at,
    }));
  const activityComments = syncedComments.length > 0 ? syncedComments : issue.comments;
  const activityEvents = syncedActivity.filter((activity) => activity.kind !== "comment");
  const subissueMutation = useMutation({
    mutationFn: (input: { title: string; body: string; parent_issue_id?: string }) => createIssue(projectId, input),
    onSuccess: () => {
      setSubissueDialogOpen(false);
      refresh();
    },
  });

  return (
    <>
      <ProjectHeader
        view="all"
        views={[]}
        onViewChange={() => undefined}
        showNotifications={false}
        content={<div className="flex min-w-0 items-center gap-3 text-xs">
          <Link to="/$organizationSlug/teams/$teamKey/issues" params={{ organizationSlug, teamKey }} className="inline-flex shrink-0 items-center gap-1 text-muted-foreground hover:text-foreground"><ArrowLeft className="size-3.5" /> Issues</Link>
          <span className="text-muted-foreground/40">›</span>
          <span className="truncate font-mono text-muted-foreground">{issue.display_key}</span>
          <span className="truncate text-muted-foreground/70">{title}</span>
          <Badge variant="outline" className="shrink-0 capitalize">{issue.status.replaceAll("_", " ")}</Badge>
        </div>}
        actions={<>
          <span className="px-2 text-[11px] text-muted-foreground">{mutation.isPending ? "Saving…" : hasDraftChanges ? "Saving soon…" : descriptionSyncState === "connecting" || descriptionSyncState === "reconnecting" ? "Syncing…" : descriptionSyncState === "error" ? "Sync unavailable" : "Saved"}</span>
          <IssueImportanceMenu importance={importance} disabled={mutation.isPending} onChange={(nextImportance) => { setImportance(nextImportance); mutation.mutate({ importance: nextImportance }); }} />
          <IssueStatusMenu status={issue.status} disabled={mutation.isPending} onChange={(status) => mutation.mutate({ status })} />
        </>}
      />
      <div className="mx-auto grid w-full max-w-6xl gap-6 px-4 py-5 sm:px-6 sm:py-6 lg:px-8">
      <div className="grid min-w-0 gap-6 lg:gap-10 lg:grid-cols-[minmax(0,1fr)_220px]">
        <main className="min-w-0">
          <div className="grid gap-4">
        <RichTextTitleEditor value={title} onChange={setTitle} />
        {descriptionDocumentQuery.data && descriptionVersionQuery.data ? (
          <DocumentEditor
            key={`issue-description-editor-${descriptionSchemaVersion}`}
            value={descriptionContent}
            onChange={setDescriptionContent}
            loroSession={descriptionSession ?? undefined}
            schemaVersion={descriptionSchemaVersion}
          />
        ) : (
          <div className="grid gap-2">
            <p className="text-xs text-muted-foreground">This description is waiting for its document binding and is currently read-only.</p>
            <RichTextBodyEditor value={body} onChange={() => undefined} editable={false} />
          </div>
        )}
          </div>
          {mutation.error ? <span className="text-xs text-destructive">{mutation.error.message}</span> : null}
          {syncConflict ? <div className="flex items-center justify-between gap-3 rounded-md border border-orange-400/30 bg-orange-400/5 px-3 py-2 text-xs">
            <span>This issue changed elsewhere. Keep your draft or use the server version.</span>
            <div className="flex shrink-0 gap-2">
              <Button size="sm" className="h-11 sm:h-7" variant="outline" onClick={() => { setSyncConflict(null); mutation.mutate({ title: title.trim() }); }}>Keep my draft</Button>
              <Button size="sm" className="h-11 sm:h-7" onClick={() => { setTitle(syncConflict.title); setBody(syncConflict.body); setImportance(syncConflict.importance); setSpecComplete(syncConflict.spec_complete); setSyncConflict(null); }}>Use server version</Button>
            </div>
          </div> : null}
      <section className="grid gap-3 border-t border-border/60 pt-5">
        <div className="flex flex-col items-stretch gap-3 sm:flex-row sm:items-center sm:justify-between">
          <div>
            <h2 className="text-sm font-medium">Sub-issues</h2>
            <p className="text-xs text-muted-foreground">Smaller pieces of work belonging to this issue.</p>
          </div>
          {canComment ? <Button variant="outline" size="sm" className="h-11 sm:h-7" onClick={() => setSubissueDialogOpen(true)}>Add sub-issue</Button> : null}
        </div>
        {issue.children.length > 0 ? <div className="grid gap-1">
          {issue.children.map((child) => (
            <Link key={child.id} to="/$organizationSlug/teams/$teamKey/issues/$issueId" params={{ organizationSlug, teamKey, issueId: child.id }} className="flex items-center gap-3 rounded-md px-2 py-2 text-sm hover:bg-muted/35">
              <span className="w-16 shrink-0 font-mono text-xs text-muted-foreground">{child.display_key}</span>
              <span className="min-w-0 flex-1 truncate">{child.title}</span>
              <Badge variant="outline" className="shrink-0 capitalize">{child.status.replaceAll("_", " ")}</Badge>
            </Link>
          ))}
        </div> : <p className="text-xs text-muted-foreground">No sub-issues yet.</p>}
        {subissueMutation.error ? <span className="text-xs text-destructive">{subissueMutation.error.message}</span> : null}
      </section>
      {activityQuery.isPending ? <div className="border-t border-border/60 pt-5 text-xs text-muted-foreground">Loading activity…</div> : null}
      <IssueActivityTimeline
        comments={activityComments}
        activities={activityEvents.length > 0 ? activityEvents : activityQuery.data ?? []}
        canComment={canComment}
        submitting={commentMutation.isPending}
        error={activityQuery.error ?? commentMutation.error ?? undefined}
        onSubmit={(content) => commentMutation.mutate(content)}
      />
      </main>
      <aside className="grid content-start gap-3 text-sm">
      <details className="group rounded-lg border border-border/60 bg-card/20 p-3">
        <summary className="cursor-pointer list-none text-xs font-medium marker:hidden">Ownership and recovery</summary>
        <div className="mt-3 grid gap-3">
        <div className="flex flex-col items-stretch gap-3 sm:flex-row sm:items-center sm:justify-between">
          <div>
            <p className="text-xs text-muted-foreground">
              {issue.active_lease_id
                ? `Agent session ${issue.active_owner_session_id?.slice(0, 8) ?? "unknown"} currently owns this lease.`
                : "No active agent lease."}
            </p>
          </div>
          {issue.active_lease_id ? (
            <div className="grid gap-2 sm:flex sm:items-center">
              <Input aria-label="Takeover reason" value={takeoverReason} onChange={(event) => setTakeoverReason(event.target.value)} placeholder="Reason for takeover" className="h-11 w-full text-xs sm:h-8 sm:w-52" />
              <Button size="sm" variant="destructive" className="h-11 sm:h-8" onClick={() => takeoverMutation.mutate()} disabled={takeoverMutation.isPending || !takeoverReason.trim()}>
                {takeoverMutation.isPending ? "Taking over..." : "Take over"}
              </Button>
            </div>
          ) : null}
        </div>
        {checklist ? (
            <div className="flex flex-col gap-3 rounded-md border border-orange-400/30 bg-orange-400/5 px-3 py-3 text-xs sm:flex-row sm:items-center sm:justify-between">
            <div className="grid gap-1"><span>Recovery checklist open: {checklist.reason}</span><span className="text-muted-foreground">Actions apply against issue version {issue.version}.</span></div>
            <div className="grid gap-2 sm:flex">
              <Button size="sm" className="h-11 sm:h-8" variant="outline" onClick={() => recoveryMutation.mutate("release")} disabled={recoveryMutation.isPending}>Reopen for dispatch</Button>
              <Button size="sm" className="h-11 sm:h-8" onClick={() => recoveryMutation.mutate("complete")} disabled={recoveryMutation.isPending}>Complete recovery</Button>
            </div>
          </div>
        ) : null}
        {takeoverMutation.error || recoveryMutation.error ? <span className="text-xs text-destructive">{(takeoverMutation.error ?? recoveryMutation.error)?.message}</span> : null}
        {recoveryFeedback ? <p role={recoveryFeedback.state === "rejected" ? "alert" : "status"} className={recoveryFeedback.state === "rejected" ? "text-xs text-destructive" : recoveryFeedback.state === "pending" ? "text-xs text-muted-foreground" : "text-xs text-emerald-400"}>{recoveryFeedback.message}</p> : null}
        </div>
      </details>
      {canApprove && issue.active_lease_id ? (
        <details className="group rounded-lg border border-border/60 bg-card/20 p-3">
          <summary className="cursor-pointer list-none text-xs font-medium marker:hidden">Collaborators</summary>
          <div className="mt-3 grid gap-3">
            <p className="text-xs text-muted-foreground">Delegate a bounded capability under the current lease fence.</p>
          <div className="grid gap-2 sm:flex sm:flex-wrap sm:items-center">
            <select aria-label="Collaborator session" value={collaboratorSessionId} onChange={(event) => setCollaboratorSessionId(event.target.value)} className="h-11 min-w-0 w-full rounded-md border border-input bg-background px-2 text-xs sm:h-8 sm:w-auto sm:min-w-48">
              <option value="">Choose active session</option>
              {projectRoster?.sessions.filter((session) => session.state === "active").map((session) => <option key={session.id} value={session.id}>{session.id.slice(0, 12)}</option>)}
            </select>
            <select aria-label="Collaborator capability" value={collaboratorCapability} onChange={(event) => setCollaboratorCapability(event.target.value)} className="h-11 min-w-0 w-full rounded-md border border-input bg-background px-2 text-xs sm:h-8 sm:w-auto">
              {['comment', 'request_spec', 'discover', 'complete', 'release', 'edit_issue', 'manage_relationships', 'recovery_review'].map((capability) => <option key={capability} value={capability}>{capability}</option>)}
            </select>
            <select aria-label="Collaborator grant mode" value={collaboratorMode} onChange={(event) => setCollaboratorMode(event.target.value as typeof collaboratorMode)} className="h-11 min-w-0 w-full rounded-md border border-input bg-background px-2 text-xs sm:h-8 sm:w-auto">
              <option value="auto">Automatic</option><option value="approval_required">Approval required</option>
            </select>
            <Button size="sm" variant="outline" className="h-11 sm:h-8" onClick={() => grantCollaboratorMutation.mutate()} disabled={grantCollaboratorMutation.isPending || !collaboratorSessionId}>Grant</Button>
          </div>
          {issue.collaborators.filter((collaborator) => !collaborator.revoked_at).map((collaborator) => (
            <div key={`${collaborator.session_id}-${collaborator.capability}`} className="flex items-center justify-between text-xs">
              <span><span className="font-mono">{collaborator.session_id.slice(0, 12)}</span><span className="ml-2 text-muted-foreground">{collaborator.capability} · {collaborator.grant_mode}</span></span>
              <Button size="sm" variant="ghost" className="h-11 sm:h-7 text-destructive" onClick={() => revokeCollaboratorMutation.mutate({ sessionId: collaborator.session_id, capability: collaborator.capability })} disabled={revokeCollaboratorMutation.isPending}>Revoke</Button>
            </div>
          ))}
          {grantCollaboratorMutation.error || revokeCollaboratorMutation.error ? <span className="text-xs text-destructive">{(grantCollaboratorMutation.error ?? revokeCollaboratorMutation.error)?.message}</span> : null}
          </div>
        </details>
      ) : null}
      <details className="group rounded-lg border border-border/60 bg-card/20 p-3">
        <summary className="cursor-pointer list-none text-xs font-medium marker:hidden">Approval request</summary>
        <div className="mt-3 grid gap-3">
          <p className="text-xs text-muted-foreground">Propose rank {proposedRank} against issue version {issue.version}. Approval applies only if that version is still current.</p>
        <div className="flex flex-wrap items-center gap-2">
          <Input aria-label="Proposed rank" value={proposedRank} onChange={(event) => setProposedRank(event.target.value)} className="h-11 w-24 text-xs sm:h-8" type="number" min="0" />
          <Button size="sm" className="h-11 sm:h-8" variant="outline" onClick={() => approvalMutation.mutate()} disabled={approvalMutation.isPending || !Number.isInteger(Number(proposedRank))}>Request approval</Button>
          {approval ? <Badge variant={approval.state === "approved" ? "default" : "outline"}>Request {approval.state}</Badge> : null}
          {approval?.state === "pending" && canApprove ? <Button size="sm" className="h-11 sm:h-7" onClick={() => decisionMutation.mutate(true)} disabled={decisionMutation.isPending}>Approve</Button> : null}
          {approval?.state === "pending" && canApprove ? <Button size="sm" className="h-11 sm:h-7" variant="ghost" onClick={() => decisionMutation.mutate(false)} disabled={decisionMutation.isPending}>Reject</Button> : null}
        </div>
        {approvalMutation.error || decisionMutation.error ? <span className="text-xs text-destructive">{(approvalMutation.error ?? decisionMutation.error)?.message}</span> : null}
        {approvalFeedback ? <p role={approvalFeedback.state === "rejected" ? "alert" : "status"} className={approvalFeedback.state === "rejected" ? "text-xs text-destructive" : approvalFeedback.state === "pending" ? "text-xs text-muted-foreground" : "text-xs text-emerald-400"}>{approvalFeedback.message}</p> : null}
        </div>
      </details>
      <details className="group rounded-lg border border-border/60 bg-card/20 p-3">
        <summary className="cursor-pointer list-none text-xs font-medium marker:hidden">Triage</summary>
        <div className="mt-3 grid gap-4">
          <p className="text-xs text-muted-foreground">Manage holds and blocking relationships.</p>
        <div className="grid gap-2 sm:flex sm:flex-wrap sm:items-center">
          <Input aria-label="Hold reason" value={holdReason} onChange={(event) => setHoldReason(event.target.value)} placeholder="Reason for hold" className="h-11 w-full text-xs sm:h-8 sm:w-56" />
          <Button size="sm" className="h-11 sm:h-8" variant="outline" onClick={() => holdMutation.mutate()} disabled={holdMutation.isPending || !holdReason.trim()}>Place hold</Button>
        </div>
        <div className="grid gap-2 sm:flex sm:flex-wrap sm:items-center">
          <select aria-label="Blocking issue" value={blockerIssueId} onChange={(event) => setBlockerIssueId(event.target.value)} className="h-11 min-w-0 w-full rounded-md border border-input bg-background px-2 text-xs sm:h-8 sm:w-auto sm:min-w-56">
            <option value="">Choose a blocking issue</option>
            {allIssuesQuery.data?.filter((candidate) => candidate.id !== issue.id).map((candidate) => <option key={candidate.id} value={candidate.id}>{candidate.display_key} · {candidate.title}</option>)}
          </select>
          <Button size="sm" className="h-11 sm:h-8" variant="outline" onClick={() => edgeMutation.mutate()} disabled={edgeMutation.isPending || !blockerIssueId.trim()}>Add blocker</Button>
        </div>
        {issue.holds.filter((hold) => !hold.released_at).map((hold) => (
          <div key={hold.id} className="flex items-center justify-between text-xs">
            <span><Badge variant="outline">{hold.hold_type}</Badge><span className="ml-2 text-muted-foreground">{hold.reason}</span></span>
            <Button size="sm" variant="ghost" className="h-11 sm:h-7" onClick={() => releaseHoldMutation.mutate(hold.id)} disabled={releaseHoldMutation.isPending}>Release</Button>
          </div>
        ))}
        {issue.edges.filter((edge) => edge.edge_type === "blocks").map((edge) => (
          <div key={edge.id} className="flex items-center justify-between text-xs">
            <span><Badge variant="outline">blocks</Badge><span className="ml-2 font-mono text-muted-foreground">{allIssuesQuery.data?.find((candidate) => candidate.id === (edge.target_issue_id === issue.id ? edge.source_issue_id : edge.target_issue_id))?.display_key ?? "linked issue"}</span></span>
            <Button size="sm" variant="ghost" className="h-11 sm:h-7" onClick={() => removeEdgeMutation.mutate(edge.id)} disabled={removeEdgeMutation.isPending}>Remove</Button>
          </div>
        ))}
        {holdMutation.error || edgeMutation.error || releaseHoldMutation.error || removeEdgeMutation.error ? <span className="text-xs text-destructive">{(holdMutation.error ?? edgeMutation.error ?? releaseHoldMutation.error ?? removeEdgeMutation.error)?.message}</span> : null}
        </div>
      </details>
      {canApprove && issue.quarantined_attempt_count > 0 ? (
        <details className="group rounded-lg border border-border/60 bg-card/20 p-3">
          <summary className="cursor-pointer list-none text-xs font-medium marker:hidden">Quarantined attempts</summary>
          <div className="mt-3 grid gap-2">
          {quarantineQuery.isPending ? <span className="text-xs text-muted-foreground">Loading recovery data...</span> : null}
          {quarantineQuery.data?.map((attempt) => (
            <div key={attempt.id} className="rounded-md border border-border/70 p-3 text-xs">
              <div className="flex justify-between"><span>{attempt.reason}</span><span className="font-mono text-muted-foreground">{attempt.session_id.slice(0, 8)}</span></div>
              <pre className="mt-2 max-h-40 overflow-auto whitespace-pre-wrap text-[10px] text-muted-foreground">{JSON.stringify(attempt.payload, null, 2)}</pre>
            </div>
          ))}
          {quarantineQuery.error ? <span className="text-xs text-destructive">{quarantineQuery.error.message}</span> : null}
          </div>
        </details>
      ) : null}
      <div className="border-t border-border/60 pt-4 text-xs text-muted-foreground">
        <div className="flex items-center gap-2"><CircleDot className="size-3.5" /> Rank {issue.rank}<span>·</span><time dateTime={issue.updated_at} title={new Date(issue.updated_at).toLocaleString()}>Last edited {formatRelativeTime(issue.updated_at, relativeTimeNow)}</time></div>
        {issue.unresolved_blocker_count > 0 || issue.active_hold_count > 0 ? <p className="mt-2">{issue.unresolved_blocker_count} blockers · {issue.active_hold_count} holds</p> : null}
        {issue.quarantined_attempt_count > 0 ? <p className="mt-2">{issue.quarantined_attempt_count} quarantined attempt{issue.quarantined_attempt_count === 1 ? "" : "s"}</p> : null}
        {issue.approvals.filter((request) => request.state === "pending").length > 0 ? <p className="mt-2">{issue.approvals.filter((request) => request.state === "pending").length} pending approval{issue.approvals.filter((request) => request.state === "pending").length === 1 ? "" : "s"}</p> : null}
      </div>
      <section className="rounded-lg border border-border/60 bg-card/20 p-3 text-xs">
        <h2 className="mb-3 font-medium text-muted-foreground">Specification</h2>
        <div className="grid gap-3">
          <div className="flex items-center justify-between gap-3">
            <span className={specComplete && !issue.specification_changed_since_review ? "text-emerald-400" : "text-orange-400"}>
              {specComplete ? (issue.specification_changed_since_review ? "Specification changed" : "Ready for dispatch") : "Needs specification"}
            </span>
            <Button size="sm" className="h-11 sm:h-7" variant="outline" onClick={() => { const next = !specComplete || issue.specification_changed_since_review; setSpecComplete(next); mutation.mutate({ spec_complete: next }); }} disabled={mutation.isPending}>
              {specComplete ? (issue.specification_changed_since_review ? "Mark reviewed" : "Reopen") : "Mark ready"}
            </Button>
          </div>
          <p className="text-muted-foreground">
            {specComplete && issue.specification_changed_since_review
              ? "The description changed after the last review. Mark it ready again when the requirements are clear."
              : "Agents can claim this issue only after its requirements are clear."}
          </p>
        </div>
      </section>
      <section className="rounded-lg border border-border/60 bg-card/20 p-3 text-xs">
        <h2 className="mb-3 font-medium text-muted-foreground">Details</h2>
        <div className="grid gap-2.5">
          <div className="flex items-center justify-between gap-3"><span className="text-muted-foreground">Team</span><span className="font-mono">{issue.display_key.split("-")[0]}</span></div>
          <div className="flex items-center justify-between gap-3"><span className="text-muted-foreground">Project</span><span className="truncate">{issue.projects.find((project) => project.project_id === projectId)?.project_name ?? "Unassigned"}</span></div>
          <div className="flex items-start justify-between gap-3"><span className="pt-0.5 text-muted-foreground">Labels</span>{issue.labels.length > 0 ? <div className="flex flex-wrap justify-end gap-1">{issue.labels.map((label) => <Badge key={label} variant="secondary">{label}</Badge>)}</div> : <span className="text-muted-foreground/60">None</span>}</div>
        </div>
      </section>
        </aside>
      </div>
      </div>
      <LazyIssueCreateDialog open={subissueDialogOpen} onOpenChange={setSubissueDialogOpen} parentIssueId={issue.id} onSubmit={(input) => subissueMutation.mutate(input)} submitting={subissueMutation.isPending} />
    </>
  );
}

export function IssueDetailPage() {
  const { organizationSlug, teamKey, issueId } = useParams({ from: "/$organizationSlug/teams/$teamKey/issues/$issueId" });
  const navigate = useNavigate();
  const appLogout = useAppLogout();
  const meQuery = useQuery({ queryKey: ["auth", "me"], queryFn: getCurrentUser, retry: false });
  const navigationQuery = useNavigation();
  const organization = navigationQuery.data?.organizations.find((candidate) => toOrganizationSlug(candidate.name) === organizationSlug);
  const { activeMembership, projectId, selectProject } = useActiveProject(meQuery.data?.memberships);
  const projectName = activeMembership?.project_name ?? "riichi";
  const issueQuery = useQuery({
    queryKey: ["issue", "global", issueId],
    queryFn: () => getGlobalIssue(issueId),
    enabled: Boolean(issueId),
  });
  const issueProjectId = issueQuery.data?.project_id ?? projectId;
  const metadataCollection = useMemo(
    () => issueProjectId ? createIssueMetadataCollection(issueProjectId) : null,
    [issueProjectId],
  );
  const metadataQuery = useLiveQuery(() => metadataCollection, [metadataCollection]);
  const syncedMetadata = metadataQuery.data?.find((candidate) => candidate.id === issueId);
  const visibleIssue = issueQuery.data && syncedMetadata
    ? { ...issueQuery.data, ...syncedMetadata }
    : issueQuery.data;
  const error = meQuery.error ?? issueQuery.error;
  const role = issueProjectId
    ? meQuery.data?.memberships.find((membership) => membership.project_id === issueProjectId)?.role
    : undefined;
  const canApprove = role === "owner" || role === "admin";
  const canComment = role === "owner" || role === "admin" || role === "member";

  return (
    <ProjectShell sidebar={<ProjectSidebar projectName={projectName} navigation={navigationQuery.data} memberships={meQuery.data?.memberships} activeProjectId={projectId} onProjectChange={selectProject} onLogout={appLogout} avatarUrl={meQuery.data?.avatar_url} onSearch={() => undefined} onNavigate={(label) => {
      if (label === "Issues") void navigate({ to: "/$organizationSlug/issues", params: { organizationSlug } });
      if (label === "Agents") void navigate({ to: "/$organizationSlug/agents", params: { organizationSlug } });
      if (label === "Link GitHub") void navigate({ to: "/$organizationSlug/integrations", params: { organizationSlug } });
      if (label === "Invite people") void navigate({ to: "/$organizationSlug/settings", params: { organizationSlug } });
    }} userName={meQuery.data?.display_name ?? "Alex Morgan"} />}>
      {issueQuery.isPending ? <div className="grid flex-1 place-items-center"><LoaderCircle className="size-5 animate-spin text-muted-foreground" /></div> : null}
      {error ? <div className="p-8 text-sm text-destructive">{error instanceof ApiError && error.status === 401 ? <a href="/auth/login" className="underline">Sign in</a> : error.message}</div> : null}
      {visibleIssue && issueProjectId && organization ? <IssueEditor issue={visibleIssue} projectId={issueProjectId} organizationSlug={organizationSlug} organizationId={organization.id} teamKey={teamKey} canApprove={canApprove} canComment={canComment} metadataCollection={metadataCollection} /> : null}
    </ProjectShell>
  );
}

export function LegacyIssueDetailRedirect() {
  const { issueId } = useParams({ from: "/issues/$issueId" });
  const navigate = useNavigate();
  const navigationQuery = useNavigation();
  const issueQuery = useQuery({
    queryKey: ["issue", "legacy-redirect", issueId],
    queryFn: () => getGlobalIssue(issueId),
    enabled: Boolean(issueId),
  });
  useEffect(() => {
    const displayKey = issueQuery.data?.display_key;
    if (!displayKey) return;
    const teamKey = navigationQuery.data?.organizations
      .flatMap((organization) => organization.teams)
      .find((team) => displayKey.startsWith(`${team.key}-`))?.key
      ?? displayKey.slice(0, displayKey.lastIndexOf("-"));
    const organizationName = navigationQuery.data?.organizations[0]?.name ?? "Riichi";
    void navigate({ to: "/$organizationSlug/teams/$teamKey/issues/$issueId", params: { organizationSlug: toOrganizationSlug(organizationName), teamKey, issueId }, replace: true });
  }, [issueQuery.data, issueId, navigate, navigationQuery.data]);

  if (issueQuery.error) return <div className="p-8 text-sm text-destructive">{issueQuery.error.message}</div>;
  return <div className="grid min-h-svh place-items-center text-sm text-muted-foreground"><LoaderCircle className="size-5 animate-spin" /></div>;
}
