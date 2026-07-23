import { useState } from "react";
import { useMutation, useQuery } from "@tanstack/react-query";

import { SettingsShell } from "@/components/settings/settings-shell";
import { ApiError, createInvite, getCurrentUser } from "@/lib/api";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useActiveProject } from "@/hooks/use-active-project";

export function SettingsProjectPage() {
  const [email, setEmail] = useState("");
  const [role, setRole] = useState<"viewer" | "member" | "admin">("member");
  const meQuery = useQuery({ queryKey: ["auth", "me"], queryFn: getCurrentUser, retry: false });
  const { activeMembership } = useActiveProject(meQuery.data?.memberships);
  const canManageProject = activeMembership?.role === "owner" || activeMembership?.role === "admin";
  const inviteMutation = useMutation({ mutationFn: () => createInvite(activeMembership!.project_id, { role, email_hint: email.trim() || undefined }), onSuccess: () => setEmail("") });

  if (!activeMembership) return <SettingsShell><p className="text-sm text-muted-foreground">Loading active project…</p></SettingsShell>;
  return <SettingsShell><header className="mb-8"><p className="text-xs text-muted-foreground">Active project</p><h1 className="mt-2 text-2xl font-medium tracking-tight">{activeMembership.project_name}</h1><p className="mt-2 text-sm text-muted-foreground">Access, invitations, and integrations for this project.</p></header><div className="grid max-w-2xl gap-8">{canManageProject ? <section className="grid gap-5 rounded-lg border border-border/70 bg-card/20 p-5"><div><h2 className="text-sm font-medium">Project members</h2><p className="mt-1 text-xs text-muted-foreground">Invite someone to this project with a bounded role.</p></div><div className="grid gap-3 sm:grid-cols-[minmax(0,1fr)_7rem_auto] sm:items-end"><label className="grid gap-1.5 text-xs text-muted-foreground">Email hint<Input aria-label="Invite email" value={email} onChange={(event) => setEmail(event.target.value)} placeholder="person@example.com" className="h-8 text-xs" /></label><label className="grid gap-1.5 text-xs text-muted-foreground">Role<select aria-label="Invite role" value={role} onChange={(event) => setRole(event.target.value as typeof role)} className="h-8 rounded-md border border-input bg-background px-2 text-xs"><option value="viewer">Viewer</option><option value="member">Member</option><option value="admin">Admin</option></select></label><Button size="sm" onClick={() => inviteMutation.mutate()} disabled={inviteMutation.isPending}>{inviteMutation.isPending ? "Creating…" : "Create invite"}</Button></div>{inviteMutation.error ? <span className="text-xs text-destructive">{inviteMutation.error instanceof ApiError && inviteMutation.error.status === 403 ? "Admin access is required." : inviteMutation.error.message}</span> : null}{inviteMutation.data ? <div className="grid gap-2 rounded-md border border-border/70 p-3 text-xs"><Badge variant="secondary">Invite created</Badge><span className="text-muted-foreground">Share this one-time token with the invitee:</span><code className="break-all rounded bg-muted px-2 py-1">{inviteMutation.data.token}</code></div> : null}</section> : <p className="text-xs text-muted-foreground">You have {activeMembership.role} access to this project. Project admins can manage members and integrations.</p>}<section className="grid gap-3 rounded-lg border border-border/70 bg-card/20 p-5"><div><h2 className="text-sm font-medium">Integrations</h2><p className="mt-1 text-xs text-muted-foreground">Connect external systems to this project.</p></div><p className="text-xs text-muted-foreground">GitHub integration settings will live here.</p></section></div></SettingsShell>;
}
