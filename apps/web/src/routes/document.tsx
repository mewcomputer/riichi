import { useEffect, useMemo, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, useNavigate, useParams } from "@tanstack/react-router";
import { ArrowLeft, FileText, Layers3, Plus } from "@/lib/product-icons";

import type { CommandMenuGroup } from "@/components/command/command-menu";
import { DocumentEditor, type DocumentEditorHandle } from "@/components/documents/document-editor";
import { DocumentRelationships } from "@/components/documents/document-relationships";
import { teamMarkLabel } from "@/components/team/team-mark";
import { extractDocumentReferences } from "@/components/documents/document-references";
import type { ResourceLinkItem } from "@/components/documents/resource-list";
import { LazyIssueCreateDialog } from "@/components/issues/lazy-issue-create-dialog";
import {
  createIssue,
  ApiError,
  getAllIssues,
  getDocument,
  getDocumentBacklinks,
  getDocumentReferences,
  getDocumentVersion,
  getDocumentLoroSnapshot,
  documentLoroWebSocketUrl,
  attachmentUrl,
  completeAttachmentUpload,
  createAttachmentUpload,
  listOrganizationDocuments,
  listProjectDocuments,
  listTeamDocuments,
  putAttachmentUpload,
  replaceDocumentReferences,
  updateDocumentMetadata,
  updateDocumentContent,
} from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Kbd } from "@/components/ui/kbd";
import { ProjectHeader } from "@/components/project/project-header";
import { ProjectShell } from "@/components/project/project-shell";
import { ProjectSidebar } from "@/components/project/project-sidebar";
import { getCurrentUser } from "@/lib/api";
import { useAppLogout } from "@/hooks/use-app-logout";
import { useActiveProject } from "@/hooks/use-active-project";
import { useNavigation } from "@/hooks/use-navigation";
import { useHumanDocuments } from "@/hooks/use-human-documents";
import { organizationSlug as toOrganizationSlug } from "@/lib/organization-slug";
import { LoroDocumentPersistence } from "@/lib/loro-persistence";
import { LoroDocumentSession, type LoroSyncState } from "@/lib/loro-document";
import { normalizeDocumentContent } from "@/lib/document-content";

const emptyContent = normalizeDocumentContent({ type: "doc", content: [] });

function serializeReferences(references: Array<{ source_block_id: string; resource_kind: string; resource_id: string; reference_kind: string }>) {
  return JSON.stringify(references.map(({ source_block_id, resource_kind, resource_id, reference_kind }) => ({
    source_block_id,
    resource_kind,
    resource_id,
    reference_kind,
  })));
}

export function DocumentPage() {
  const { organizationSlug, documentId } = useParams({ from: "/$organizationSlug/documents/$documentId" });
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const appLogout = useAppLogout();
  const meQuery = useQuery({ queryKey: ["auth", "me"], queryFn: getCurrentUser, retry: false });
  const navigationQuery = useNavigation();
  const syncedDocuments = useHumanDocuments();
  const organization = navigationQuery.data?.organizations.find((candidate) => toOrganizationSlug(candidate.name) === organizationSlug);
  const { activeMembership, projectId, selectProject } = useActiveProject(meQuery.data?.memberships);
  const resourceIssuesQuery = useQuery({
    queryKey: ["issues", "all"],
    queryFn: () => getAllIssues(),
    enabled: Boolean(organization),
  });
  const resourceDocumentsQuery = useQuery({
    queryKey: ["documents", "resource-links", organization?.id],
    queryFn: async () => {
      if (!organization) return [];
      const results = await Promise.allSettled([
        listOrganizationDocuments(organization.id),
        ...organization.teams.flatMap((team) => [
          listTeamDocuments(team.id),
          ...team.projects.map((project) => listProjectDocuments(project.id)),
        ]),
      ]);
      return [...new Map(
        results.flatMap((result) => result.status === "fulfilled" ? result.value : [])
          .map((document) => [document.id, document]),
      ).values()];
    },
    enabled: Boolean(organization),
  });
  const documentQuery = useQuery({
    queryKey: ["document", documentId],
    queryFn: () => getDocument(documentId),
  });
  const versionQuery = useQuery({
    queryKey: ["document", documentId, "version"],
    queryFn: () => getDocumentVersion(documentId),
    enabled: Boolean(documentQuery.data),
  });
  const referencesQuery = useQuery({
    queryKey: ["document", documentId, "references"],
    queryFn: () => getDocumentReferences(documentId),
    enabled: Boolean(documentQuery.data),
  });
  const backlinksQuery = useQuery({
    queryKey: ["document", documentId, "backlinks"],
    queryFn: () => getDocumentBacklinks(documentId),
    enabled: Boolean(documentQuery.data),
  });
  const resourceItems = useMemo<ResourceLinkItem[]>(() => {
    const teams = organization?.teams ?? [];
    return [
      ...teams.map((team) => ({
        id: team.id,
        label: `${teamMarkLabel(team.emoji)} ${team.name}`,
        description: `${team.key} team`,
        kind: "team" as const,
        href: `/${organizationSlug}/teams/${team.key}`,
      })),
      ...teams.flatMap((team) => team.projects.map((project) => ({
        id: project.id,
        label: project.name,
        description: `${team.key} project`,
        kind: "project" as const,
        href: `/${organizationSlug}/projects/${project.id}`,
      }))),
      ...(resourceIssuesQuery.data ?? []).map((issue) => ({
        id: issue.id,
        label: `${issue.display_key} · ${issue.title}`,
        description: `${issue.team_key} · ${issue.project_name}`,
        kind: "issue" as const,
        href: `/${organizationSlug}/teams/${issue.team_key}/issues/${issue.id}`,
      })),
      ...(syncedDocuments ?? resourceDocumentsQuery.data ?? []).map((resourceDocument) => ({
        id: resourceDocument.id,
        label: resourceDocument.title,
        description: resourceDocument.kind.replaceAll("_", " "),
        kind: "document" as const,
        href: `/${organizationSlug}/documents/${resourceDocument.id}`,
      })),
    ];
  }, [organization, organizationSlug, resourceDocumentsQuery.data, resourceIssuesQuery.data, syncedDocuments]);
  const [content, setContent] = useState<Record<string, unknown>>(emptyContent);
  const [savedContent, setSavedContent] = useState("");
  const [title, setTitle] = useState("");
  const [createOpen, setCreateOpen] = useState(false);
  const [attachmentError, setAttachmentError] = useState<string | null>(null);
  const [loroSession, setLoroSession] = useState<LoroDocumentSession | null>(null);
  const [loroSessionLoading, setLoroSessionLoading] = useState(false);
  const [loroSyncState, setLoroSyncState] = useState<LoroSyncState>("disconnected");
  const [schemaVersion, setSchemaVersion] = useState(1);
  const [savedReferences, setSavedReferences] = useState("");
  const attachmentInput = useRef<HTMLInputElement>(null);
  const documentEditor = useRef<DocumentEditorHandle>(null);
  const loroPersistence = useRef<LoroDocumentPersistence | null>(null);
  const initializedVersionDocumentId = useRef<string | null>(null);
  const initializedReferencesDocumentId = useRef<string | null>(null);

  useEffect(() => {
    if (!versionQuery.data || initializedVersionDocumentId.current === documentId) return;
    initializedVersionDocumentId.current = documentId;
    const nextContent = normalizeDocumentContent(versionQuery.data.content);
    const serialized = JSON.stringify(nextContent);
    setContent(nextContent);
    setSavedContent(serialized);
  }, [documentId, versionQuery.data]);

  useEffect(() => {
    if (!referencesQuery.data || initializedReferencesDocumentId.current === documentId) return;
    initializedReferencesDocumentId.current = documentId;
    setSavedReferences(serializeReferences(referencesQuery.data));
  }, [documentId, referencesQuery.data]);

  useEffect(() => {
    if (documentQuery.data) setTitle(documentQuery.data.title);
  }, [documentQuery.data]);

  useEffect(() => {
    const organization = navigationQuery.data?.organizations.find(
      (candidate) => toOrganizationSlug(candidate.name) === organizationSlug,
    );
    if (!organization || !documentQuery.data) return;
    let disposed = false;
    setLoroSessionLoading(true);
    const persistence = loroPersistence.current ?? new LoroDocumentPersistence();
    loroPersistence.current = persistence;
    let session: LoroDocumentSession | undefined;
    void (async () => {
      try {
        const serverSnapshot = await getDocumentLoroSnapshot(documentId);
        setSchemaVersion(serverSnapshot.schema_version);
        const opened = await LoroDocumentSession.open({
          persistence,
          scope: { organizationId: organization.id, documentId },
          serverSnapshot: new Uint8Array(serverSnapshot.bytes),
          serverSchemaVersion: serverSnapshot.schema_version,
        });
        session = opened;
        if (disposed) {
          await opened.dispose();
          return;
        }
        setLoroSyncState("connecting");
        setLoroSession(opened);
        setLoroSessionLoading(false);
        try {
          await opened.connectWebSocket(documentLoroWebSocketUrl(documentId), undefined, setLoroSyncState, async () => {
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
          if (!disposed) setLoroSyncState("error");
        }
      } catch {
        if (!disposed) {
          setLoroSessionLoading(false);
          setLoroSyncState("error");
        }
      }
    })();
    return () => {
      disposed = true;
      setLoroSessionLoading(false);
      setLoroSession((current) => {
        if (current === session) return null;
        return current;
      });
      void session?.dispose();
    };
  }, [documentId, documentQuery.data?.id, navigationQuery.data, organizationSlug]);

  const mutation = useMutation({
    mutationFn: (nextContent: Record<string, unknown>) => {
      const revision = versionQuery.data?.revision;
      if (revision === undefined) throw new Error("Document revision is unavailable");
      return updateDocumentContent(documentId, {
        expected_revision: revision,
        content: nextContent,
        references: extractDocumentReferences(nextContent),
      });
    },
    onSuccess: (updated, nextContent) => {
      setSavedContent(JSON.stringify(nextContent));
      queryClient.setQueryData(["document", documentId], updated);
      void queryClient.invalidateQueries({ queryKey: ["document", documentId, "version"] });
    },
  });
  const metadataMutation = useMutation({
    mutationFn: (nextTitle: string) =>
      updateDocumentMetadata(documentId, {
        title: nextTitle,
        parent_document_id: documentQuery.data?.parent_document_id,
        position: documentQuery.data?.position,
      }),
    onSuccess: (updated) => {
      setTitle(updated.title);
      queryClient.setQueryData(["document", documentId], updated);
    },
  });
  const referencesMutation = useMutation({
    mutationFn: (references: ReturnType<typeof extractDocumentReferences>) => replaceDocumentReferences(documentId, references),
    onSuccess: (references) => {
      setSavedReferences(serializeReferences(references));
      queryClient.setQueryData(["document", documentId, "references"], references);
    },
  });
  const attachmentMutation = useMutation({
    mutationFn: async (file: File) => {
      const digest = await crypto.subtle.digest("SHA-256", await file.arrayBuffer());
      const checksum = Array.from(new Uint8Array(digest), (byte) => byte.toString(16).padStart(2, "0")).join("");
      const upload = await createAttachmentUpload(documentId, {
        filename: file.name,
        media_type: file.type || "application/octet-stream",
        byte_size: file.size,
        checksum,
        source_block_id: `attachment-${crypto.randomUUID()}`,
      });
      await putAttachmentUpload(upload.upload_id, file);
      return completeAttachmentUpload(upload.upload_id);
    },
    onSuccess: (attachment) => {
      setAttachmentError(null);
      documentEditor.current?.insertAttachment({
        src: attachmentUrl(attachment.id),
        alt: attachment.filename,
        attachmentId: attachment.id,
      });
    },
    onError: (error) => setAttachmentError(error instanceof Error ? error.message : "Could not upload attachment"),
  });
  const createIssueMutation = useMutation({
    mutationFn: (input: { title: string; body: string }) => {
      if (!projectId) throw new Error("No project membership is available.");
      return createIssue(projectId, input);
    },
    onSuccess: (issue) => {
      setCreateOpen(false);
      void queryClient.invalidateQueries({ queryKey: ["issues", "all"] });
      void navigate({
        to: "/$organizationSlug/teams/$teamKey/issues/$issueId",
        params: { organizationSlug, teamKey: issue.team_key, issueId: issue.id },
      });
    },
  });

  useEffect(() => {
    const serialized = JSON.stringify(content);
    if (loroSession || loroSessionLoading || !versionQuery.data || serialized === savedContent || mutation.isPending) return;
    const timeout = window.setTimeout(() => mutation.mutate(content), 700);
    return () => window.clearTimeout(timeout);
  }, [content, loroSession, loroSessionLoading, mutation, savedContent, versionQuery.data]);

  useEffect(() => {
    if (!loroSession || loroSessionLoading || !referencesQuery.isSuccess || referencesMutation.isPending) return;
    const nextReferences = extractDocumentReferences(content);
    const serialized = serializeReferences(nextReferences);
    if (serialized === savedReferences) return;
    const timeout = window.setTimeout(() => referencesMutation.mutate(nextReferences), 700);
    return () => window.clearTimeout(timeout);
  }, [content, loroSession, loroSessionLoading, referencesMutation, referencesQuery.isSuccess, savedReferences]);

  const document = documentQuery.data;
  if (documentQuery.isPending || versionQuery.isPending) {
    return <div className="p-8 text-sm text-muted-foreground">Loading document…</div>;
  }
  if (!document || !versionQuery.data) {
    return <div className="p-8 text-sm text-muted-foreground">Document unavailable.</div>;
  }
  const ownerTeam = organization?.teams.find(
    (team) => team.id === document.owner_team_id || team.projects.some((project) => project.id === document.owner_project_id),
  );
  const projectName = activeMembership?.project_name ?? "riichi";
  const commandGroups: CommandMenuGroup[] = [
    {
      id: "document-navigation",
      label: "Navigate",
      items: [
        {
          id: "create-issue",
          label: "New issue",
          icon: Plus,
          shortcut: "C",
          onSelect: () => setCreateOpen(true),
        },
        {
          id: "all-issues",
          label: "All issues",
          icon: Layers3,
          shortcut: "G I",
          onSelect: () => void navigate({ to: "/$organizationSlug/issues", params: { organizationSlug } }),
        },
        {
          id: "close-document",
          label: "Close document",
          icon: ArrowLeft,
          onSelect: () => void navigate({ to: "/$organizationSlug/issues", params: { organizationSlug } }),
        },
      ],
    },
  ];

  return (
    <ProjectShell
      commandGroups={commandGroups}
      sidebar={
        <ProjectSidebar
          projectName={projectName}
          navigation={navigationQuery.data}
          memberships={meQuery.data?.memberships}
          activeProjectId={projectId}
          onProjectChange={selectProject}
          onLogout={appLogout}
          avatarUrl={meQuery.data?.avatar_url}
          onSearch={() => window.dispatchEvent(new Event("riichi:open-command-menu"))}
          onCreate={() => setCreateOpen(true)}
          onNavigate={(label) => {
            if (label === "Agents") void navigate({ to: "/$organizationSlug/agents", params: { organizationSlug } });
            if (label === "Link GitHub") void navigate({ to: "/$organizationSlug/integrations", params: { organizationSlug } });
            if (label === "Invite people") void navigate({ to: "/$organizationSlug/settings", params: { organizationSlug } });
          }}
          userName={meQuery.data?.display_name ?? "Alex Morgan"}
        />
      }
      footer={
        <footer className="flex h-8 shrink-0 items-center gap-3 border-t border-border/60 px-4 text-[10px] text-muted-foreground">
          <span className="flex items-center gap-1.5 text-foreground/70">
            <span className={`size-1.5 rounded-full ${loroSyncState === "error" ? "bg-destructive" : "bg-emerald-400"}`} />
            {loroSyncState === "error" ? "Sync needs attention" : "Document synced"}
          </span>
          <span className="ml-auto">{document.title}</span>
          <Kbd className="h-5 bg-muted px-1.5 text-[10px]">⌘ K</Kbd>
          <span>Command menu</span>
        </footer>
      }
    >
      <ProjectHeader
        view="all"
        views={[]}
        onViewChange={() => undefined}
        content={
          <div className="flex min-w-0 items-center gap-2 text-xs">
            <Link to="/$organizationSlug/issues" params={{ organizationSlug }} className="text-muted-foreground hover:text-foreground">
              {organization?.name ?? organizationSlug}
            </Link>
            <span className="text-muted-foreground/40">›</span>
            {ownerTeam ? (
              <Link
                to="/$organizationSlug/teams/$teamKey"
                params={{ organizationSlug, teamKey: ownerTeam.key }}
                className="text-muted-foreground hover:text-foreground"
              >
                Notes
              </Link>
            ) : (
              <Link to="/$organizationSlug/issues" params={{ organizationSlug }} className="text-muted-foreground hover:text-foreground">
                Notes
              </Link>
            )}
            <span className="text-muted-foreground/40">›</span>
            <span className="truncate font-medium" aria-current="page">{document.title}</span>
            <FileText className="size-3.5 shrink-0 text-muted-foreground" />
          </div>
        }
        actions={
          <>
            <span className="px-2 text-[11px] text-muted-foreground">
              {mutation.isPending || metadataMutation.isPending || referencesMutation.isPending || attachmentMutation.isPending
                ? "Saving…"
                : mutation.error || metadataMutation.error || referencesMutation.error || attachmentError
                  ? "Could not save"
                  : loroSyncState === "reconnecting" || loroSyncState === "connecting"
                    ? "Syncing…"
                    : loroSyncState === "error"
                      ? "Sync unavailable"
                      : "Saved"}
            </span>
            <Button type="button" variant="ghost" size="sm" onClick={() => void navigate({ to: "/$organizationSlug/issues", params: { organizationSlug } })}>
              Close
            </Button>
          </>
        }
      />
      <main className="min-h-0 flex-1 overflow-y-auto">
        <div className="mx-auto w-full max-w-3xl px-8 py-12">
        <input
          value={title}
          onChange={(event) => setTitle(event.target.value)}
          onBlur={() => {
            const nextTitle = title.trim();
            if (!nextTitle) {
              setTitle(document.title);
              return;
            }
            if (nextTitle === document.title) return;
            metadataMutation.mutate(nextTitle);
          }}
          aria-label="Document title"
          className="mb-8 w-full bg-transparent text-3xl font-medium tracking-tight outline-none"
        />
        <input
          ref={attachmentInput}
          type="file"
          className="hidden"
          onChange={(event) => {
            const file = event.target.files?.[0];
            event.target.value = "";
            if (file) attachmentMutation.mutate(file);
          }}
        />
        <DocumentEditor
          key={`document-editor-${schemaVersion}`}
          ref={documentEditor}
          value={content}
          onChange={setContent}
          loroSession={loroSession ?? undefined}
          schemaVersion={schemaVersion}
          mentionItems={meQuery.data ? [{
            id: meQuery.data.account_id,
            label: meQuery.data.display_name ?? meQuery.data.email ?? "You",
            description: meQuery.data.email ?? undefined,
          }] : []}
          resourceItems={resourceItems}
          onRequestAttachment={() => attachmentInput.current?.click()}
        />
        <DocumentRelationships
          organizationSlug={organizationSlug}
          navigation={navigationQuery.data}
          references={referencesQuery.data ?? []}
          backlinks={backlinksQuery.data ?? []}
        />
        </div>
      </main>
      <LazyIssueCreateDialog
        open={createOpen}
        onOpenChange={setCreateOpen}
        onSubmit={(input) => createIssueMutation.mutate(input)}
        submitting={createIssueMutation.isPending}
      />
    </ProjectShell>
  );
}
