# Riichi familiarity and interaction optimization RFC

**Status:** accepted

## 1. Summary

Riichi is working well for early teams, but teams already fluent in Linear
bring strong expectations about navigation, issue handling, keyboard use, and
queue density. This RFC defines a bounded familiarity pass that reduces the
cost of learning Riichi without copying Linear's planning model or weakening
Riichi's agent-dispatch controls.

The target audience is design-partner teams who already manage day-to-day work
in a Linear-like issue tracker and now need to triage, dispatch, supervise,
and recover agent work. The interface should feel fast, dense, predictable,
and trustworthy.

## 2. Decision

Prioritize familiar interaction patterns and terminology before adding new
planning entities.

Cycles, milestones, estimates, velocity, capacity planning, and timeboxing
remain deferred. They may be revisited later if teams show a repeated need for
time-bounded planning rather than merely familiar navigation.

## 2.1 Implementation status

The first implementation slices preserve the server-authoritative write
boundary while adding familiar navigation and feedback patterns:

| Slice | Evidence | State |
| --- | --- | --- |
| Queue filters, URL state, keyboard movement, and saved views | Queue search tests, shortcut-reference contract coverage, account-owned saved-view API, and generated API contract | shipped |
| Per-item queue and issue mutation acknowledgement | Queue, issue-detail, and approval feedback states with frontend coverage; bulk results retain row-level acknowledgements and show confirmed/rejected totals | shipped |
| Actionable inbox and approval lifecycle state | Server-enriched approval state scoped to current project access | shipped |
| First-use terminology hints | Dismissible issue-detail hint for lease, fencing token, and approval | shipped |
| Guided sample workspace with agent claim/report and recovery fixtures | Admin-only onboarding-sample command creates PostgreSQL-backed issues, a role/session, fenced claim/report history, approval, and recovery checklist; authenticated API integration coverage verifies the response, persisted marker, and idempotent repeat | shipped |

## 3. Goals

- Let an experienced issue-tracker user find and act on an issue quickly.
- Make queue state, filters, ownership, approvals, and agent health easy to
  understand at a glance.
- Make common actions discoverable by mouse, keyboard, and command menu.
- Preserve Riichi's server-authoritative leases, approvals, fencing, and
  recovery semantics.
- Improve adoption without requiring teams to learn a new planning hierarchy.

## 4. Non-goals

- Do not add a cycle or timebox table as part of this RFC.
- Do not add estimates, velocity, capacity, sprint rollover, or automatic
  issue movement.
- Do not make agent state look like ordinary human assignment state.
- Do not hide lease ownership, pending approvals, stale actions, or recovery
  warnings behind a familiar visual pattern.
- Do not implement broad Linear import or migration until the core workflow is
  measurably easier to use.

## 5. Product principles

### Familiar shell, Riichi authority

Use conventions users already know: issue URLs, breadcrumbs, list density,
property sidebars, inline edits, filter chips, keyboard shortcuts, and a
command menu. Keep Riichi-specific concepts explicit where they affect
authority: active lease, fencing state, pending approval, quarantine, and
recovery status.

### Optimize the next action

Every queue and issue-detail surface should make the next useful action clear:
triage, claim, inspect, approve, recover, reopen, or report. Secondary
metadata should not compete with that action.

### Progressive disclosure

Show ordinary issue work first. Reveal lease diagnostics, quarantined payloads,
approval details, and recovery controls to the roles that can use them.

### No optimistic authority

The UI may feel immediate, but confirmed state comes from the named API
command and its replication or transaction acknowledgement. Pending,
confirmed, rejected, stale, and superseded states must remain distinguishable.

## 6. Experience workstreams

### P0: navigation and terminology

- Use consistent labels for issue, status, priority, assignee, team, project,
  labels, approval, lease, and recovery.
- Keep stable, shareable issue URLs and meaningful breadcrumbs.
- Make browser back/forward behavior preserve the user's queue and filters.
- Ensure the command menu exposes the same actions as visible controls.
- Add a shortcut reference reachable from the command menu.

### P0: queue ergonomics

- Add visible filter chips for project, team, status, priority, assignee, and
  labels.
- Support saved views or named filter presets without introducing a cycle
  entity.
- Support keyboard movement between issues and opening the selected issue.
- Support multi-select for safe bulk operations: status, priority, assignee,
  labels, and archive/cancel where authorization allows.
- Keep a compact “my work” or “needs my decision” view for approvals,
  takeovers, and recovery.
- Preserve clear empty, loading, stale, and rejected states.

### P1: issue-detail flow

- Keep title, status, priority, assignee, labels, relationships, and project
  context in a predictable property region.
- Make inline edits reversible and show their authoritative acknowledgement.
- Add hover or keyboard previews where they reduce context switching.
- Keep agent lease health visible but secondary to the issue itself.
- Make approval and recovery actions typed, role-aware, and explicit about
  their resulting status.

### P1: inbox and feedback

- Consolidate mentions, approvals, takeovers, lease events, failed reports,
  and recovery requests into one actionable inbox.
- Link each item directly to the issue and preserve the originating action.
- Show whether an action is pending, confirmed, rejected, superseded, or
  expired.
- Avoid notification noise for ordinary replicated read-model updates.

### P2: onboarding and migration friction

- Provide a small guided sample workspace that demonstrates human triage,
  agent claim/report, approval, and recovery.
- Offer terminology hints only at first use; do not add permanent explanatory
  chrome.
- Evaluate a bounded Linear import only after the core workflow metrics improve.

## 7. Interaction contract

The first implementation pass should establish these behaviors:

1. A user can open the queue, filter to actionable work, select an issue, and
   perform the next authorized action without leaving the primary context.
2. A keyboard user can move through the queue, open an issue, invoke the
   command menu, and return to the prior view without losing position.
3. A bulk action reports pending, confirmed, rejected, and partially rejected
   results per issue rather than presenting one misleading global success.
4. An approval or recovery action explains its target version and resulting
   state before submission, then shows authoritative acknowledgement.
5. A user who lacks access sees the existence or redacted status required by
   policy, not a leaked quarantined payload or internal permission detail.
6. On refresh or reconnect, the UI reconstructs the same queue view and any
   open recovery state from the server rather than local component state.

## 8. Performance and accessibility targets

- Queue navigation should remain responsive with hundreds of visible issues.
- Keyboard focus must be visible and never trapped by a popover or command
  surface.
- Every shortcut has a pointer-accessible equivalent.
- Destructive, external, and authority-changing actions require clear labels
  and confirmation appropriate to their risk.
- Reduced-motion preferences must disable nonessential transitions.
- Mobile and narrow layouts must retain issue identity, status, and the next
  authorized action; secondary metadata can collapse.

## 9. Measurement

Establish a design-partner baseline before implementation and compare after
the familiarity pass:

- time from queue entry to first useful action;
- time from issue open to status, assignment, or comment action;
- percentage of repeated users using keyboard or command-menu actions;
- filter/view reuse and time to return to a prior view;
- bulk-action completion and rejection rates;
- clarification requests caused by unclear ownership or availability;
- recovery and approval completion time;
- self-reported familiarity and confidence after onboarding.

Do not optimize for raw click reduction if it increases stale mutations,
approval mistakes, or recovery confusion.

## 10. Rollout sequence

1. Audit current queue, command-menu, issue-detail, inbox, and responsive
   behavior against the interaction contract.
2. Ship terminology, URL, keyboard, filter, saved-view, and focus fixes.
3. Add safe bulk actions and the actionable inbox.
4. Test with the existing design partners and compare the baseline metrics.
5. Add onboarding improvements and decide whether a bounded import is worth
   the integration cost.
6. Revisit cycles only if at least three teams independently request
   time-bounded planning and the evidence shows navigation alone is not the
   underlying problem.

## 11. Technical boundaries

- Read models may add filter and view projections, but issue authority remains
  in PostgreSQL commands.
- Saved views must be user/project configuration, not a new dispatch state.
- Keyboard and command-menu actions must call the same named API commands as
  visible controls.
- Bulk actions need per-item idempotency, typed rejection handling, and
  transaction-marker or authoritative reconciliation behavior.
- No cycle schema, cycle API, cycle permissions, or cycle-specific agent
  protocol is introduced by this RFC.

## 12. Open questions

- Which shortcut map best fits Riichi's existing command menu and browser
  conventions?
- Should saved views be personal only in the first pass, or shareable within a
  project?
- Which bulk actions are safe enough for one-step execution versus requiring
  confirmation?
- Should the inbox be a global account surface or scoped to the active project?
- What minimum mobile behavior is required for the current pilot cohort?
