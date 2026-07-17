import { useMemo } from "react";
import { useQueries, useQuery } from "@tanstack/react-query";
import { Link2 } from "lucide-react";

import type { DocumentRecord, DocumentReference, HumanQueueIssue, NavigationResponse } from "@/lib/api";
import { getAllIssues, getDocument } from "@/lib/api";

type DocumentRelationshipsProps = {
  organizationSlug: string;
  navigation?: NavigationResponse;
  references: DocumentReference[];
  backlinks: DocumentReference[];
};

export function DocumentRelationships({ organizationSlug, navigation, references, backlinks }: DocumentRelationshipsProps) {
  const issueReferenceIds = useMemo(
    () => references.filter((reference) => reference.resource_kind === "issue").map((reference) => reference.resource_id),
    [references],
  );
  const issuesQuery = useQuery({
    queryKey: ["issues", "all"],
    queryFn: () => getAllIssues(),
    enabled: issueReferenceIds.length > 0,
  });
  const documentIds = useMemo(
    () => [...new Set([
      ...references.filter((reference) => reference.resource_kind === "document").map((reference) => reference.resource_id),
      ...backlinks.map((reference) => reference.document_id),
    ])],
    [backlinks, references],
  );
  const documentQueries = useQueries({
    queries: documentIds.map((documentId) => ({
      queryKey: ["document", documentId],
      queryFn: () => getDocument(documentId),
    })),
  });
  const documentsById = new Map<string, DocumentRecord>();
  documentQueries.forEach((query, index) => {
    if (query.data) documentsById.set(documentIds[index], query.data);
  });

  const referenceItems = references
    .map((reference) => resolveReference(reference, organizationSlug, navigation, issuesQuery.data ?? [], documentsById))
    .filter((item): item is RelationshipItem => item !== null);
  const backlinkItems = backlinks
    .map((reference) => {
      const document = documentsById.get(reference.document_id);
      if (!document) return null;
      return {
        key: `backlink-${reference.document_id}-${reference.source_block_id}`,
        label: document.title,
        href: `/${organizationSlug}/documents/${document.id}`,
      } satisfies RelationshipItem;
    })
    .filter((item): item is RelationshipItem => item !== null);

  if (referenceItems.length === 0 && backlinkItems.length === 0) return null;

  return (
    <div className="mt-10 grid gap-6 border-t border-border/60 pt-6 text-xs">
      {referenceItems.length ? <RelationshipGroup label="References" items={referenceItems} /> : null}
      {backlinkItems.length ? <RelationshipGroup label="Backlinks" items={backlinkItems} /> : null}
    </div>
  );
}

type RelationshipItem = { key: string; label: string; href: string };

function RelationshipGroup({ label, items }: { label: string; items: RelationshipItem[] }) {
  return (
    <section className="grid gap-2">
      <h2 className="flex items-center gap-2 font-medium text-muted-foreground"><Link2 className="size-3.5" />{label}</h2>
      <div className="grid gap-1">
        {items.map((item) => <a key={item.key} href={item.href} className="truncate text-muted-foreground underline-offset-4 hover:text-foreground hover:underline">{item.label}</a>)}
      </div>
    </section>
  );
}

function resolveReference(
  reference: DocumentReference,
  organizationSlug: string,
  navigation: NavigationResponse | undefined,
  issues: HumanQueueIssue[],
  documentsById: Map<string, DocumentRecord>,
): RelationshipItem | null {
  const key = `reference-${reference.resource_kind}-${reference.resource_id}-${reference.source_block_id}`;
  if (reference.resource_kind === "document") {
    const document = documentsById.get(reference.resource_id);
    return document ? { key, label: document.title, href: `/${organizationSlug}/documents/${document.id}` } : null;
  }
  if (reference.resource_kind === "issue") {
    const issue = issues.find((candidate) => candidate.id === reference.resource_id);
    return issue ? { key, label: `${issue.display_key} · ${issue.title}`, href: `/${organizationSlug}/teams/${issue.team_key}/issues/${issue.id}` } : null;
  }
  const teams = navigation?.organizations.flatMap((organization) => organization.teams) ?? [];
  if (reference.resource_kind === "team") {
    const team = teams.find((candidate) => candidate.id === reference.resource_id);
    return team ? { key, label: `${team.emoji ?? "◈"} ${team.name}`, href: `/${organizationSlug}/teams/${team.key}` } : null;
  }
  const projectScope = teams.flatMap((team) => team.projects.map((project) => ({ project, team }))).find(({ project }) => project.id === reference.resource_id);
  return projectScope ? { key, label: projectScope.project.name, href: `/${organizationSlug}/projects/${projectScope.project.id}` } : null;
}
