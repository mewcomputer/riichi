# Riichi hierarchy

Riichi uses “workspace” as a user-facing access scope: everything the signed-in person can see across the organization. It is not a parent container that owns issues.

```text
Organization
├── global Inbox / My issues / Reviews
├── global Projects / Views
└── Teams
    ├── Issues
    ├── Projects
    └── Views

Team owns issues
Project groups issues, including issues from multiple teams
Issue may be attached to zero or more projects
```

## Organization

The organization is the security and collaboration boundary. Riichi has one organization during the pilot, while the schema keeps the boundary for a future multi-organization product.

It owns teams, projects, organization memberships, and organization-level policy. The compact `R Riichi` header is the organization identity in the sidebar.

## Workspace access scope

The application workspace is computed from the signed-in user’s organization, team, project, and explicit membership access. It should not require a context switch to move between teams or projects.

Global surfaces aggregate only records the server has authorized:

- All Issues: issues across accessible teams.
- My Issues: issues assigned to the current human.
- Projects: accessible projects across teams.
- Views: saved filters over the same authorized universe.
- Reviews and Inbox: durable human workflows, when enabled.

## Teams

A team owns an issue namespace and the operational people/agent collaboration around those issues. Team keys should be stable and issue identifiers should eventually be team-scoped, for example `RII-42`.

Each team exposes the same navigation shape:

- Issues: the team’s issue list and triage views.
- Projects: projects that include one or more of the team’s issues.
- Views: saved team-scoped filters.

Teams can participate in many projects. A project can include issues from many teams.

## Projects

A project is a cross-team grouping and coordination container. It does not own an issue namespace.

The relationship is many-to-many:

```text
Team A ── owns ──> Issue A ── attached to ──> Project X
Team B ── owns ──> Issue B ── attached to ──> Project X
Team C ── owns ──> Issue C ── attached to ──> Project Y
```

The `issue_projects` join table is authoritative for attachments. An issue can exist without a project, and can be attached to multiple projects. Cross-project issue references remain a separate relationship from project membership.

## Issues

An issue has exactly one owning team. Its title, body, lifecycle, dispatch state, leases, holds, approvals, comments, and activity belong to that issue and therefore to its owning team.

An issue may be attached to multiple projects for planning and reporting. Those attachments must not change the issue’s team ownership or dispatch authority.

## Access rules

Authorization should resolve in this order:

1. authenticate the human or agent;
2. resolve the issue’s owning team and organization;
3. resolve any project attachment involved in the operation;
4. require the capability for the operation;
5. perform the transaction server-side.

Project membership can grant access to attached issues according to project policy, but it must not silently grant team administration or change issue ownership. Team membership controls issue operations and team settings. Organization membership controls organization settings.

## Current migration boundary

Migration `0016_team_owned_issues.sql` adds `issues.team_id` and the many-to-many `issue_projects` table, backfilling existing pilot issues from their current project/team relationship. Migration `0018_team_owned_agent_runtime.sql` adds team ownership to agent roles and sessions and changes dispatch rank scope to `team`.

Existing project-scoped operational routes remain as compatibility surfaces during the transition. Their database checks now resolve team ownership for agent roles and sessions, while project IDs remain available for routing, integrations, and historical reporting.

The next backend migration should move issue creation, issue authorization, dispatch, and issue detail reads to team ownership. Project-scoped agent and integration APIs should then be explicitly classified as either team-owned or project metadata instead of inheriting the old workspace model.
