# Riichi post-pilot product direction RFC

**Status:** proposed
**Scope:** product direction after the pilot, not detailed implementation
**Related:** [pilot PRD](./riichi-pilot-prd.md), [pilot architecture RFC](./riichi-pilot-architecture-rfc.md), [familiarity and interaction optimization RFC](./riichi-familiarity-optimization-rfc.md)

## 1. Summary

Riichi should become a familiar, fast issue tracker that makes human-and-agent
work safe, observable, and recoverable.

The product should feel immediately usable to teams familiar with Linear-like
issue tracking: dense queues, predictable issue handling, keyboard navigation,
filters, saved views, previews, and clear history. Its reason to exist is the
operational layer underneath that familiar shell: leases, approvals, fencing,
holds, collaborator capabilities, quarantine, and recovery remain explicit and
server-authoritative.

This RFC sets the post-pilot feature order. It prioritizes improvements that
make existing Riichi state easier to operate before adding a new planning
model. It deliberately defers cycles, estimates, velocity, capacity planning,
roadmaps, and arbitrary custom fields until repeated demand justifies them.

## 2. Context

The pilot tests a narrow wedge: a shared queue, bounded context, exclusive
leases, and human controls for teams coordinating multiple coding-agent
sessions. The existing model already includes most of the state needed for a
useful operating surface:

- issues, priorities, assignment, parent and child work, projects, labels, and
  typed relationships;
- blockers, holds, leases, fencing tokens, collaborators, approvals, comments,
  and recovery checklists;
- queue filters, keyboard navigation, saved views, bulk mutation
  acknowledgement, an actionable approval inbox, terminology hints, and a
  guided agent workflow.

The next product risk is therefore not a lack of entities. It is that users
cannot quickly understand or act on the state Riichi already keeps. Adding a
second planning system before solving that problem would increase surface area
without strengthening the wedge.

## 3. Decision

Adopt the following product direction:

1. Preserve a familiar issue-tracker shell and interaction model.
2. Make agent ownership, approvals, blockers, handoffs, and recovery visible
   in ordinary issue workflows.
3. Exploit existing domain models before introducing new planning entities.
4. Add configuration only when it has a stable canonical meaning for dispatch,
   authorization, history, and reporting.
5. Treat every authority-changing action as an explicit, acknowledged,
   auditable command.
6. Revisit heavyweight planning features only after evidence shows that the
   operating workflow is insufficient without them.

The product story becomes:

> familiar issue handling.
> clear agent ownership.
> every approval, blocker, handoff, and recovery in view.

## 4. Product principles

### Familiar shell, Riichi authority

Use established issue-tracker conventions for navigation, density, properties,
filters, views, keyboard shortcuts, and issue history. Keep active leases,
fencing state, approvals, quarantine, and recovery distinct from ordinary
assignment or status so users do not mistake a familiar presentation for a
different authority model.

### Make operational state actionable

An issue should answer what is happening, who owns it, what is blocking it, and
what the next authorized action is. A project should answer what needs human
attention, what agents are doing, and where work is stalled.

### Use existing state before inventing concepts

Typed relationships, holds, activity, approvals, and leases should carry more
of the product experience before Riichi adds a broad decision graph or another
planning hierarchy.

### Progressive disclosure without hiding risk

Keep ordinary issue work compact. Reveal lease diagnostics, quarantined
payloads, approval details, and recovery controls to the roles that can use
them. Never hide a stale, pending, rejected, superseded, or authority-changing
state merely to make the interface look familiar.

### Reversible human control

Ordinary property changes should be recoverable through version-checked
commands. Agent actions and authority changes should be attributable to their
role and session, with compensating actions rather than edits to history.

## 5. Prioritized product work

### P0: turn the existing queue into an operating surface

#### 5.1 Issue peek and split-pane navigation

Selecting an issue from a queue should open a right-hand peek pane without
losing queue context. The interaction must preserve:

- scroll position and selected rows;
- active filters and sort order;
- keyboard focus and neighboring issue context;
- a stable URL that can be copied and revisited.

The initial keyboard contract is:

- `j` / `k`: move through the queue;
- `enter`: open the selected issue in the peek pane;
- `esc`: close the pane and restore queue focus;
- an explicit shortcut: expand the issue to its full page.

The full issue page and peek pane must use the same authoritative data and
commands. Peek is a navigation optimization, not a separate issue state.

#### 5.2 Shared and pinned views

Extend account-owned saved views into scoped views:

- personal views;
- team views;
- project views;
- admin-defined defaults;
- pinned sidebar views;
- copying a shared view into personal views.

Useful first views include:

- needs human approval;
- unowned urgent work;
- agent-ready backlog;
- blocked by another team;
- leases expiring today;
- failed or quarantined agent work.

View ownership, visibility, and default behavior must be explicit. A view is a
query and presentation configuration, not a dispatch state. Notification rules
for views are deferred until view permissions and query stability are proven.

#### 5.3 Relationship UX

Make the existing relationship model useful before adding new relationship
types:

- inline `blocked by` and `blocks` sections;
- dependency indicators in queue rows;
- parent and child navigation;
- visible `discovered from` provenance for agent-created issues;
- duplicate merge as an explicit, reviewable operation;
- warnings when closing an issue that still blocks active work.

Relationship changes must retain their current validation and authorization.
The UI may make a relationship easier to discover, but it must not infer that
a dependency exists from labels or text.

#### 5.4 Understandable history and undo

Render authoritative activity in human language, for example:

- status changed from todo to blocked;
- agent eligibility disabled;
- assignment moved from one owner to another;
- dependency added;
- approval superseded;
- lease expired;
- agent created three child issues.

For ordinary property changes, offer a version-checked revert command where an
inverse operation is well-defined. For agent actions, show actor role, session,
source issue, affected objects, and confirmation state. Audit records remain
append-only; undo creates a new compensating action.

### P1: add lightweight time and workflow fit

#### 5.5 Dates and snoozing, without cycles

Add small, composable time semantics:

- due date;
- optional start date;
- snooze until;
- project target date;
- needs-attention-by time for approvals;
- scheduled agent eligibility.

These should support views such as due this week, overdue, starting soon,
snoozed, and approvals expiring today. They must not imply sprint rollover,
velocity accounting, or capacity planning.

#### 5.6 Adaptable workflows with canonical mappings

Teams should be able to use team-facing workflow labels while Riichi retains
stable internal lifecycle categories for dispatch, reporting, permissions, and
agent commands. An initial mapping may look like this:

| Team-facing state | Canonical category |
| --- | --- |
| design review | started |
| ready to merge | started |
| awaiting customer | blocked |
| shipped | completed |

The exact canonical vocabulary remains an implementation decision. The
constraint is that presentation labels cannot fork dispatch semantics. Workflow
configuration must be versioned so historical issues remain intelligible after
a team changes its labels or mappings.

The first implementation may provide presentation aliases over the current
status set. A new workflow schema should be introduced only when aliases no
longer cover observed needs.

#### 5.7 Issue templates and recurring work

Templates should capture the operational semantics that make Riichi different
from a basic issue tracker:

- description structure;
- labels and priority;
- child issues and relationships;
- agent eligibility;
- specification requirements;
- approval requirements;
- default holds;
- responsible team;
- expected completion evidence.

Initial template examples include security review, release checklist,
dependency upgrade, incident follow-up, design implementation, and customer
escalation.

Recurring work should instantiate a versioned template on a schedule. Each
instantiation must be idempotent and retain the template version that produced
it. Recurrence is deferred until one-off templates have a clear ownership and
permission model.

#### 5.8 Actionable subscriptions

After shared views are stable, allow users or teams to subscribe to narrowly
defined operational events such as approval requests, lease expiry, blocked
dependencies, or quarantine creation. Subscriptions should produce actionable
notifications with a direct issue link, not a second unread queue that users
must manage.

### P2: improve project and developer workflow visibility

#### 5.9 Project overview

Provide a project page that answers:

- what is moving;
- what is blocked;
- what needs a human;
- what agents are handling;
- which leases are stale or expiring;
- which approvals are pending;
- what changed recently;
- what has no owner;
- what is approaching its target date.

This is a read model over existing issues, relationships, holds, leases,
approvals, activity, and sessions. It should report observable state and
uncertainty rather than claim to understand project health beyond its data.

#### 5.10 GitHub workflow integration

Extend the existing GitHub issue import and webhook boundary into a developer
workflow:

- link issues to pull requests and commits;
- show review and CI state on the issue;
- optionally transition issues when pull requests merge;
- import labels and assignees through reviewable mappings;
- detect likely duplicates between GitHub and Riichi;
- expose agent-created branches or pull requests as issue activity.

GitHub remains an external, untrusted source. Mappings and automatic
transitions require explicit configuration, auditability, and a way to inspect
or reject the proposed change.

Slack intake and notifications may follow if design partners show more demand
for conversation-to-work capture than for a broad tracker import. A broad
Linear importer remains out of scope for this direction.

## 6. Explicitly deferred

The following are deliberately deferred:

- cycles, sprints, and timeboxes;
- estimates, velocity, capacity planning, and sprint rollover;
- portfolio roadmaps and enterprise planning hierarchies;
- arbitrary custom fields;
- a general-purpose decision graph or automatic decision extraction;
- broad Linear import and migration;
- automation rules that can silently change dispatch or authority.

These may become valid later. They require repeated evidence that lightweight
dates, views, relationships, templates, and project read models cannot solve the
observed problem.

## 7. Technical and authority boundaries

### Read models before new authority

Peek panes, shared views, dependency summaries, project overviews, and history
rendering should begin as read-model and frontend work. They must use the same
named commands as existing controls and must not create a second client-side
source of truth.

### Configuration must preserve canonical semantics

Shared views, workflow mappings, templates, and subscriptions are scoped
configuration. They require explicit ownership, authorization, versioning, and
audit records. They cannot alter lease fencing, approval revalidation, or
dispatch eligibility through an implicit UI convention.

### Undo means compensating commands

Undo must check the target version and current authorization. It must refuse or
surface a conflict when later work makes the inverse unsafe. Historical audit
records are never rewritten or deleted.

### External data remains untrusted

GitHub, Slack, imported issue text, and agent-produced content may be displayed
or mapped only through existing authorization and provenance boundaries. No
external text becomes workspace policy or executable instruction by default.

### Agent state stays distinct

Assignment is a routing signal. A lease is execution ownership. Approval is
human authorization. A workflow label must not collapse these distinctions.

## 8. Measurement and decision gates

Establish a baseline with pilot teams and measure each workstream against it:

- time from queue entry to first useful action;
- time from issue open to the next authorized action;
- time to find the cause of a blocker or pending approval;
- return-to-queue time after inspecting an issue;
- use and reuse of shared or pinned views;
- duplicate attempts and stale agent mutations;
- recovery completion time and quarantine inspection rate;
- relationship-related closure conflicts;
- undo success, conflict, and rejection rates;
- percentage of project overview information that users can act on;
- requests for cycles, estimates, capacity, or roadmap features.

Do not optimize click count if it increases stale mutations, approval mistakes,
or recovery confusion. Advance from P0 to P1 when users can operate existing
agent state without repeated navigation or interpretation failures. Advance to
P2 when teams can explain project status from the product without reconstructing
it in another tracker. Reconsider deferred planning features only when at least
three teams independently request the same planning behavior and the evidence
shows that the gap is planning rather than basic visibility or time semantics.

## 9. Rollout sequence

1. Audit the current queue, issue detail, inbox, approvals, responsive behavior,
   relationships, and activity history against the familiarity RFC's
   interaction contract.
2. Ship issue peek, relationship UX, shared and pinned views, and readable
   history as the first post-pilot slices.
3. Test those slices with design partners and compare navigation, action,
   blocker, approval, and recovery metrics.
4. Add dates, snoozing, canonical workflow mappings, templates, and
   subscriptions only where P0 evidence shows a repeated need.
5. Add project overview and richer GitHub workflow integration after the core
   operating surface is being used routinely.
6. Revisit deferred planning features only at a deliberate product review using
   the decision gates above.

## 10. Decisions resolved in review

- Shared views start project-scoped. The view model must support team scope
  later without changing ownership, visibility, or query semantics.
- “Next authorized action” means the set of named commands currently allowed
  by server-side state and permissions. The UI explains why an action is
  available or unavailable; it does not invent recommendations or duplicate
  authorization logic.
- History should render explicit events for issue changes, relationships,
  leases, reports, approvals, holds, takeovers, recovery, undo, and external
  integration state. Each event includes actor, role or session when relevant,
  timestamp, target, and authoritative result.
- Duplicate resolution creates a linked record. One issue remains the
  survivor; the duplicate is marked deprecated with a `duplicate_of` link,
  remains readable, and is redirected to the survivor. History and comments
  are not silently moved.
- Initial date semantics are daily. Due and start dates use PostgreSQL
  `date`; snoozes, lease deadlines, and attention deadlines use
  `timestamptz`. Date-only values render without timezone conversion. If
  hour-level due dates become necessary, add a separate `due_at` timestamp
  rather than converting existing calendar dates to midnight UTC.
- P1 time work starts with due dates, then snoozing. Start dates wait for
  evidence that they solve a distinct problem.
- Workflow aliases are presentation labels over canonical lifecycle
  categories. The mapping is versioned, historical issues retain the version
  that applied to them, and the schema allows new canonical categories later
  without changing the agent protocol or silently reclassifying old history.
  The first release may ship a bounded alias set before teams define their
  own.
- Templates copy their operational fields as a snapshot and retain the
  producing template version for provenance. Existing issues do not stay
  linked to later template edits, and recurrence remains deferred.
- Subscriptions start as personal, narrow subscriptions for approvals, lease
  expiry, blocked dependencies, and quarantine creation. Notifications link
  directly to the actionable issue or control.

## 11. Implementation checklist

### Direction and contracts

- [x] Decide the initial shared-view scope and preserve a path to team scope.
- [x] Define “next authorized action” as server-derived command availability.
- [x] Establish the first explicit history event taxonomy and event fields.
- [x] Define duplicate deprecation as a linked, reviewable resolution.
- [x] Define date-only and timestamp storage/display semantics.
- [x] Decide P1 time ordering: due dates before snoozing, with start dates
  evidence-gated.
- [x] Define extensible, versioned workflow aliases over canonical lifecycle
  categories.
- [x] Decide that templates snapshot operational fields and subscriptions start
  as personal, narrow, actionable notifications.

### P0 implementation preparation

- [x] Audit queue, issue detail, inbox, relationships, and activity against the
  interaction contract.
- [x] Record the baseline: queue filters, URL state, keyboard movement, saved
  views, bulk acknowledgements, and a generic activity timeline already exist;
  issue selection still navigates to a full page, saved views are account-only,
  relationship state is concentrated in issue detail, and history lacks a
  stable human-readable event taxonomy.
- [x] Specify the project-scoped view schema, filter grammar, ownership, and
  permissions: project views retain the existing filter object, are visible to
  project viewers, may be created by project members, and may be deleted by
  their owner or a project admin.
- [x] Specify the history event projection and actor/session presentation: the
  existing `issue_activity_sync` read model remains authoritative, event kinds
  map to stable human labels, and role/session enrichment is shown whenever it
  is present in the activity payload.
- [x] Specify the duplicate-resolution command and deprecated-issue behavior:
  the existing typed `duplicate_of` command creates the link, the source is
  deprecated, the target survives, and both records remain readable.
- [x] Define P0 baselines and exit thresholds for navigation, blocker
  discovery, approval handling, and return-to-queue time: capture partner
  medians before rollout, require no regression in first-action time, target a
  25% reduction in blocker/approval discovery time, and require the peek
  contract to be completed without assistance by at least 80% of returning
  users.

### P0 delivery

- [x] Ship the first issue peek slice: URL-addressable selection, queue-preserving
  `enter`/`esc` navigation, and explicit `e` expansion into the full issue page.
- [x] Ship project-shared views alongside personal views.
- [x] Add pinned sidebar views as a per-user preference over personal or
  project-shared views.
- [x] Ship relationship visibility and duplicate resolution: issue detail now
  shows all linked relationships, and duplicate links identify the survivor
  or deprecated issue without moving history.
- [x] Ship readable history and version-checked compensating undo for
  unambiguous ordinary property changes; agent actions remain explicit without
  an unsafe automatic inverse.

### Later slices

- [x] Add daily dates and snoozing without introducing cycles.
- [x] Add presentation workflow aliases with canonical lifecycle mappings.
- [x] Add versioned issue templates before considering recurrence.
- [x] Add actionable subscriptions after view permissions and query stability
  are proven.
- [x] Add project overview and richer GitHub workflow integration after P0 is
  routinely used.

## 12. Open questions for review

- Who may define admin defaults, and can a project opt out?
- Which view notifications are useful enough to avoid notification fatigue?
- Should duplicate resolution support an explicit, version-checked reversal
  after the initial linked record is created?
- What GitHub transitions are safe to automate, and which require approval?
- What baseline and threshold should the pilot use for the three-team planning
  feature gate?
