import { describe, expect, it } from "vitest";

import {
  documentFromSyncRecord,
  issueFromSyncRecord,
  type HumanDocumentSyncRecord,
  type HumanIssueSyncRecord,
} from "./metadata-sync";
import { navigationFromSyncRows, type NavigationSyncRecord } from "./navigation-sync";

function row(overrides: Partial<NavigationSyncRecord>): NavigationSyncRecord {
  return {
    account_id: "account-1",
    organization_id: "org-1",
    organization_name: "Riichi",
    organization_role: "member",
    organization_has_logo: false,
    team_id: "team-1",
    team_name: "Core",
    team_key: "COR",
    team_emoji: "◈",
    project_id: "project-1",
    project_name: "Alpha",
    project_role: "viewer",
    ...overrides,
  };
}

describe("navigationFromSyncRows", () => {
  it("groups and sorts replicated rows into the navigation response", () => {
    const navigation = navigationFromSyncRows([
      row({ project_id: "project-2", project_name: "Zeta" }),
      row({ team_id: "team-2", team_name: "Design", team_key: "DSN", project_id: "project-3", project_name: "Beta" }),
      row({ project_name: "Alpha" }),
    ]);

    expect(navigation.organizations).toHaveLength(1);
    expect(navigation.organizations[0].teams.map((team) => team.key)).toEqual(["COR", "DSN"]);
    expect(navigation.organizations[0].teams[0].projects.map((project) => project.name)).toEqual(["Alpha", "Zeta"]);
    expect(navigation.organizations[0].teams[1].projects[0].role).toBe("viewer");
  });

  it("returns an empty navigation response when the account has no accessible rows", () => {
    expect(navigationFromSyncRows([])).toEqual({ organizations: [] });
  });
});

describe("documentFromSyncRecord", () => {
  it("maps the projection's document_id into the shared document record shape", () => {
    const document = documentFromSyncRecord({
      account_id: "account-1",
      document_id: "document-1",
      organization_id: "org-1",
      kind: "team_page",
      title: "Runbook",
      parent_document_id: null,
      position: 0,
      owner_team_id: "team-1",
      owner_project_id: null,
      provisioning_state: "ready",
      created_by: "account-1",
      created_at: "2026-01-01T00:00:00Z",
      updated_at: "2026-01-01T00:00:00Z",
      deleted_at: null,
      current_revision: 1,
      plain_text: "hello",
      sanitized_html: "<p>hello</p>",
      transaction_id: 1,
    } satisfies HumanDocumentSyncRecord);

    expect(document.id).toBe("document-1");
    expect(document).not.toHaveProperty("document_id");
  });
});

describe("issueFromSyncRecord", () => {
  it("maps the projection's issue_id into the shared issue record shape", () => {
    const issue = issueFromSyncRecord({
      account_id: "account-1",
      issue_id: "issue-1",
      team_id: "team-1",
      team_name: "Core",
      team_key: "COR",
      project_id: "project-1",
      project_name: "Alpha",
      display_key: "COR-1",
      title: "Fix the queue",
      body: "",
      status: "todo",
      importance: "none",
      agent_eligible: true,
      spec_complete: true,
      specification_changed_since_review: false,
      unresolved_blocker_count: 0,
      active_hold_count: 0,
      active_lease_id: null,
      lease_expires_at: null,
      created_at: "2026-01-01T00:00:00Z",
      updated_at: "2026-01-01T00:00:00Z",
      due_date: null,
      snoozed_until: null,
      rank: 1,
      dispatch_version: 1,
      assignee_account_id: null,
      labels: [],
      transaction_id: 1,
    } satisfies HumanIssueSyncRecord);

    expect(issue.id).toBe("issue-1");
    expect(issue).not.toHaveProperty("issue_id");
  });
});
