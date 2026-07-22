import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useMemo, useState } from "react";
import { useLiveQuery } from "@tanstack/react-db";
import { Link, useNavigate, useParams } from "@tanstack/react-router";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Empty, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Check, CircleDot, X } from "@/lib/product-icons";
import { decideApprovalRequest, getCurrentUser, getPendingApprovals } from "@/lib/api";
import { ProjectHeader } from "@/components/project/project-header";
import { ProjectShell } from "@/components/project/project-shell";
import { ProjectSidebar } from "@/components/project/project-sidebar";
import { useAppLogout } from "../hooks/use-app-logout";
import { useActiveProject } from "../hooks/use-active-project";
import { useNavigation } from "../hooks/use-navigation";
import { createApprovalCollection } from "@/lib/metadata-sync";

export function ApprovalsPage() {
  const navigate = useNavigate();
  const { organizationSlug } = useParams({ from: "/$organizationSlug/approvals" });
  const appLogout = useAppLogout();
  const queryClient = useQueryClient();
  const meQuery = useQuery({ queryKey: ["auth", "me"], queryFn: getCurrentUser, retry: false });
  const navigationQuery = useNavigation();
  const { activeMembership, projectId, selectProject } = useActiveProject(meQuery.data?.memberships);
  const approvalsQuery = useQuery({ queryKey: ["approvals", "pending"], queryFn: getPendingApprovals });
  const approvalCollection = useMemo(() => createApprovalCollection(), []);
  const approvalLiveQuery = useLiveQuery(() => approvalCollection, [approvalCollection]);
  const replicatedApprovals = approvalLiveQuery.isReady ? approvalLiveQuery.data : undefined;
  const [pendingApprovalIds, setPendingApprovalIds] = useState<Set<string>>(() => new Set());
  const [feedbackByApprovalId, setFeedbackByApprovalId] = useState<Record<string, { state: "confirmed" | "rejected"; message: string }>>({});
  const decisionMutation = useMutation({
    mutationFn: (input: { projectId: string; approvalId: string; approve: boolean }) => decideApprovalRequest(input.projectId, input.approvalId, input.approve),
    onMutate: ({ approvalId }) => setPendingApprovalIds((current) => new Set(current).add(approvalId)),
    onSuccess: (_result, { approvalId, approve }) => {
      setPendingApprovalIds((current) => { const next = new Set(current); next.delete(approvalId); return next; });
      setFeedbackByApprovalId((current) => ({ ...current, [approvalId]: { state: "confirmed", message: `${approve ? "Approved" : "Rejected"}. Server state is confirmed.` } }));
      void queryClient.invalidateQueries({ queryKey: ["approvals", "pending"] });
    },
    onError: (error, { approvalId }) => {
      setPendingApprovalIds((current) => { const next = new Set(current); next.delete(approvalId); return next; });
      setFeedbackByApprovalId((current) => ({ ...current, [approvalId]: { state: "rejected", message: error instanceof Error ? error.message : "Approval decision was rejected." } }));
    },
  });

  return (
    <ProjectShell sidebar={<ProjectSidebar projectName={activeMembership?.project_name ?? "riichi"} navigation={navigationQuery.data} activeProjectId={projectId} onProjectChange={selectProject} onLogout={appLogout} avatarUrl={meQuery.data?.avatar_url} onSearch={() => undefined} onNavigate={(label) => { if (label === "Issues") void navigate({ to: "/$organizationSlug/issues", params: { organizationSlug } }); if (label === "Agents") void navigate({ to: "/$organizationSlug/agents", params: { organizationSlug } }); }} userName={meQuery.data?.display_name ?? "Alex Morgan"} />}> 
      <ProjectHeader view="all" views={[]} onViewChange={() => undefined} />
      <main className="mx-auto flex w-full max-w-screen-lg flex-col gap-6 px-8 py-8">
        <div><Link to="/$organizationSlug/issues" params={{ organizationSlug }} className="text-xs text-muted-foreground hover:text-foreground">← Issues</Link><h1 className="mt-3 text-2xl font-medium tracking-tight">Approval queue</h1><p className="mt-1 text-sm text-muted-foreground">Review proposed changes across the projects you administer.</p></div>
        {approvalsQuery.isPending ? <div className="text-sm text-muted-foreground">Loading approvals…</div> : null}
        {approvalsQuery.error ? <div className="text-sm text-destructive">{approvalsQuery.error.message}</div> : null}
        {!approvalsQuery.isPending && !approvalsQuery.error && approvalsQuery.data?.length === 0 ? <Empty className="min-h-56 border-0"><EmptyHeader><EmptyMedia variant="icon"><CircleDot /></EmptyMedia><EmptyTitle>No pending approvals</EmptyTitle></EmptyHeader></Empty> : null}
        <div className="grid gap-3">
          {(replicatedApprovals ?? approvalsQuery.data)?.map((approval) => {
            const pending = pendingApprovalIds.has(approval.id);
            const feedback = feedbackByApprovalId[approval.id];
            return <article key={approval.id} className="grid gap-4 rounded-lg border border-border/70 bg-card/30 p-4">
              <div className="flex items-start justify-between gap-4"><div><Link to="/$organizationSlug/teams/$teamKey/issues/$issueId" params={{ organizationSlug, teamKey: approval.team_key, issueId: approval.issue_id }} onClick={() => selectProject(approval.project_id)} className="font-medium hover:underline">{approval.issue_title}</Link><p className="mt-1 text-xs text-muted-foreground">{approval.project_name} · requested by {approval.requested_by.slice(0, 8)} · target version {approval.target_version}</p></div><Badge variant={feedback?.state === "rejected" ? "destructive" : feedback?.state === "confirmed" ? "default" : "outline"}>{feedback?.state ?? "pending"}</Badge></div>
              <pre className="overflow-auto rounded-md bg-muted/40 p-3 text-xs">{JSON.stringify(approval.proposed_operation, null, 2)}</pre>
              <div className="flex flex-wrap items-center justify-end gap-2">{feedback ? <p role={feedback.state === "rejected" ? "alert" : "status"} className={feedback.state === "rejected" ? "mr-auto text-xs text-destructive" : "mr-auto text-xs text-emerald-400"}>{feedback.message}</p> : null}<Button variant="ghost" size="sm" onClick={() => decisionMutation.mutate({ projectId: approval.project_id, approvalId: approval.id, approve: false })} disabled={pending || feedback?.state === "confirmed"}><X /> {pending ? "Saving…" : "Reject"}</Button><Button size="sm" onClick={() => decisionMutation.mutate({ projectId: approval.project_id, approvalId: approval.id, approve: true })} disabled={pending || feedback?.state === "confirmed"}><Check /> {pending ? "Saving…" : "Approve"}</Button></div>
            </article>;
          })}
        </div>
      </main>
    </ProjectShell>
  );
}
