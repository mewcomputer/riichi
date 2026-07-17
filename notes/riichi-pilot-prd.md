# Riichi pilot PRD

**Status:** draft for pilot planning
**Purpose:** validate the shared-dispatch wedge with real software teams
**Scope:** the smallest product that lets humans prepare and control work while coding-agent sessions discover, claim, execute, and report it

## 1. Summary

Riichi is a shared coordination layer for teams running multiple coding-agent sessions against one or more repositories.

The pilot tests one narrow promise:

> a server-arbitrated work queue, bounded task context, and visible human controls help teams coordinate multiple agent sessions with less duplicate work and less recovery overhead.

The pilot must support the routine coordination loop without requiring a database console or a second issue tracker:

1. a human prepares executable work;
2. an agent requests and claims one issue;
3. Riichi grants an exclusive, renewable lease;
4. the agent retrieves bounded context and works in the repository;
5. the agent reports progress or completion;
6. humans can inspect, reprioritize, hold, clarify, revoke, and recover work.

This document is deliberately narrower than the full Riichi product plan. It does not settle the technology stack, prove event sourcing, or define a general project-management product.

## 2. Pilot decision

### Hypothesis

If Riichi gives a team a shared queue with exclusive leases, useful bounded context, and a human control surface, then a team operating several coding-agent sessions will complete work with fewer duplicate attempts and less coordination overhead than it experiences today.

### Decision the pilot must inform

After the pilot, decide whether the shared dispatcher is a strong enough wedge to justify building the complete MVP product.

The pilot should produce evidence about:

- whether teams repeatedly use Riichi for routine dispatch;
- whether exclusive claims prevent meaningful duplicate attempts;
- whether expired work can be safely resumed;
- whether context reduces repeated clarification and setup work;
- whether humans can govern the swarm from the UI;
- which product behaviors are necessary before expanding scope.

## 3. Target users

### Primary user

The engineer or technical lead operating several coding-agent sessions against one or more repositories.

### Secondary users

- developers supervising agent work;
- reviewers clarifying specifications and resolving blocked work;
- a workspace administrator managing agent credentials and permissions.

### Pilot team profile

The first cohort should be small software teams that:

- already run multiple coding-agent sessions or intend to do so during the pilot;
- have a real repository and a backlog of bounded engineering tasks;
- can identify a human responsible for triage and intervention;
- can provide baseline observations from their current coordination workflow;
- can use a pilot build and participate in weekly feedback.

The pilot is not aimed at enterprise portfolio management, configurable workflow administration, nontechnical project teams, or teams primarily seeking sprint planning and capacity management.

## 4. User problem

Teams currently coordinate agent sessions through prompts, terminal sessions, GitHub issues, and informal notes. That workflow creates predictable failure modes:

- two agents start the same task;
- agents begin blocked or underspecified work;
- every session reconstructs context from scattered sources;
- work is abandoned when a session dies;
- humans cannot see current ownership or lease health;
- sensitive actions are either broadly trusted or manually blocked;
- a late result from an old session can overwrite newer work.

Riichi needs to make ownership, context, recovery, and intervention explicit.

## 5. Goals

The pilot will:

1. prevent legitimate concurrent ownership of one issue;
2. expose a stable four-intention agent contract: `ready`, `claim`, `context`, and `report`;
3. recover work safely when an agent session disappears;
4. provide enough context for an agent to start without reconstructing the whole workspace;
5. let humans reprioritize, hold, clarify, inspect, revoke, and release work;
6. attribute every agent action to a durable agent role and an exact session;
7. support routine pilot coordination without database or tracker intervention;
8. produce trustworthy measurements for the post-pilot MVP decision.

## 6. Non-goals

The pilot will not include:

- a general Jira or Linear replacement;
- portfolio planning, capacity management, roadmaps, or configurable workflows;
- full offline mutation support or client-side arbitration;
- collaborative issue-body editing;
- a dashboard builder or broad analytics suite;
- arbitrary automation rules;
- full two-way mirroring of GitHub issues, comments, or fields;
- a marketplace or plugin system;
- proof that event sourcing is the right architecture;
- rich projects, cycles, milestones, or sprint semantics;
- automatic execution of instructions found in imported comments, issues, reviews, or messages;
- a large collection of one-agent-tool-per-route MCP endpoints.

## 7. Pilot product boundary

### 7.1 Work item model

The pilot needs a minimal issue model:

- immutable issue ID and human-readable workspace key;
- title and bounded Markdown body;
- lifecycle status: `triage | todo | in_progress | blocked | done | canceled`;
- manual rank within a defined queue;
- labels;
- optional assignee for routing and accountability;
- `agent_eligible` flag;
- server-maintained `spec_complete` result;
- creation, update, completion, and cancellation metadata;
- optimistic concurrency version.

Assignment is a routing signal. It is not execution ownership. Execution ownership is represented only by the active claim lease.

### 7.2 Relationships and holds

The pilot supports these directed relationships:

- `blocks`: the source must be resolved before the target can be dispatched;
- `related`: informational relationship;
- `discovered_from`: provenance for newly discovered work;
- `duplicate_of`: explicit duplicate resolution.

Invalid self-edges and cycles are rejected where the relationship semantics require acyclicity.

A hold prevents otherwise eligible work from appearing in `ready`. Initial hold types are:

- `manual`;
- `needs_spec`;
- `awaiting_approval`;
- `scheduled`;
- `integration`.

Each hold has a reason, creator, timestamps, and optional expiry.

### 7.3 Agent role and session

An agent role is a durable workspace-scoped identity such as `implementation` or `triage`. It has a human owner and a bounded capability set.

A session is an ephemeral runtime identity minted by an agent role. It has:

- an immutable ID;
- a maximum lifetime and heartbeat deadline;
- capabilities equal to or narrower than its role;
- state `active | expired | revoked`;
- last heartbeat and last action timestamps.

Every agent action records both the role and session. Revoking a session invalidates its credentials and leases. Revoking a role invalidates its child sessions.

### 7.4 Claim lease

At most one active exclusive lease may exist for an issue.

A lease includes:

- issue ID and session ID;
- lease ID;
- monotonically increasing fencing token for the issue;
- claim, heartbeat, and expiry timestamps;
- state `active | released | expired | revoked | completed`;
- release reason.

The server grants, renews, releases, and expires leases transactionally. A report that mutates execution state must present the current lease ID and fencing token. A late worker with an expired token cannot overwrite the newer worker's result.

Claiming automatically changes `todo` to `in_progress`.

There is one fenced owner session per lease, but a lease may have explicitly delegated collaborators. Collaborators may receive capabilities in `auto` or `approval_required` mode. A `never` capability cannot be delegated. An `approval_required` collaborator may propose an operation and review its approval request, but cannot apply it until an authorized approval is recorded. Delegation does not create a second lease or a second fencing authority, and every action remains attributed to its actual role and session.

An authorized human can forcibly take over an active lease. Takeover records a reason, invalidates the old fencing token, marks the old session interrupted, and makes the new owner visible immediately. The interrupted session should receive a control message such as “your lease was taken over; stop work and request another issue.” Any later mutation from the old session is rejected as stale and retained as quarantined attempt data when useful for recovery. The issue remains `in_progress` under the new owner unless the human explicitly changes it.

Takeover creates a visible recovery checklist: inspect the last attempt and any quarantined results, verify the current repository and Riichi state, choose whether to continue, recover, or reopen the work, and record the outcome. Users with an explicit recovery-review capability can see the full quarantined result. An `approval_required` recovery-review grant permits inspection and proposal, not application. Other collaborators can see that a quarantine exists without seeing its contents.

The initial lease defaults are provisional: a 30-minute TTL, renewal every 5 minutes, expiry after two missed renewals, and an 8-hour maximum session lifetime. The pilot may tune these values from observed agent behavior, but the expiry and reassignment bounds must be visible and measurable.

### 7.5 Approval boundary

The pilot only needs approval for actions that are outside an agent's automatic capability grants.

An approval request stores:

- requester and session;
- exact action, target, target version, and proposed payload;
- risk explanation and expiry;
- state `pending | approved | denied | expired | superseded`;
- decision maker, time, and optional reason.

Approval revalidates permissions and target versions when executed. A materially changed target supersedes the request instead of applying stale intent.

The pilot uses this default capability policy:

- **automatic:** discover ready work, claim and renew, read context, append progress or result comments, request more specification, create discovered issues, and complete an issue with a resolution summary;
- **approval required:** cancel an issue, add or remove a blocking relationship, edit existing issue content, change assignment or rank, and mutate a GitHub issue;
- **never for agents:** manage workspace membership, credentials, capabilities, or audit history; delete or rewrite historical records.

This is a starting policy for the pilot, not a permanent permission model.

### 7.6 Audit and retry behavior

Every material mutation records actor, role, session, request ID, time, and a redacted change summary. Agent and integration mutations accept an idempotency key.

Matching retries replay the original result. Reusing a key with a different request body returns an error. Secrets do not appear in context, audit payloads, or ordinary logs.

The pilot does not require a complete event-sourced domain model. It does require an authoritative current state, an append-only audit trail, and enough durable delivery state to avoid losing required notifications or integration work.

## 8. Core user journeys

### Journey A: prepare work

1. A human creates or imports an issue.
2. The issue enters `triage`.
3. Riichi identifies missing required specification, holds, and unresolved blockers.
4. A human adds clarification, relationships, rank, and any required fields.
5. The human marks the issue eligible and releases applicable holds.
6. The issue appears in `ready` only when it is dispatchable.

**Success condition:** a human can tell why an issue is not ready and can make it ready without database intervention.

### Journey B: execute work

1. A session requests ready work within its authorized scope.
2. Riichi returns eligible issues from one authoritative snapshot.
3. The session claims one issue.
4. Riichi grants an exclusive renewable lease and returns its expiry and fencing token.
5. The session retrieves a bounded context bundle.
6. The session works in the repository and sends progress or completion reports.
7. Riichi records the outcome and releases or completes the lease.

**Success condition:** two sessions cannot both become legitimate owners, and humans can see the active owner.

### Journey C: recover failed work

1. A session stops renewing its lease.
2. Riichi expires the lease within the defined detection bound.
3. The issue returns to `ready` or `blocked`, according to its current state.
4. A new session claims it.
5. The new context bundle includes relevant activity from the previous attempt.
6. A late report from the expired session is rejected or quarantined and cannot mutate authoritative state.

**Success condition:** a failed session creates recoverable work, not a permanently stuck issue or a race between workers.

### Journey D: request approval

1. An agent proposes an action outside its automatic permissions.
2. Riichi creates an approval request containing the exact mutation.
3. A human inspects and approves or denies it.
4. Riichi revalidates the target and permissions before execution.
5. A stale proposal is superseded rather than applied.

**Success condition:** sensitive actions can be governed without granting agents blanket write authority.

### Journey E: control the swarm

1. A human opens the queue or board.
2. They see active claims, lease health, blocked work, needs-spec work, and approvals.
3. They reprioritize work, add or remove a hold, clarify an issue, revoke a session, or release a lease.
4. Agents observe the resulting authoritative state.

**Success condition:** humans can control the swarm from Riichi during normal operation.

## 9. Functional requirements

### Dispatch

- **DSP-01:** `ready` returns only `todo` issues that are agent-eligible, specification-complete, unheld, unclaimed, unblocked, and authorized for the caller.
- **DSP-02:** eligibility is evaluated against authoritative server state at a declared snapshot or cursor.
- **DSP-03:** claiming is atomic and has exactly one winner.
- **DSP-04:** every claim returns a lease expiry and fencing token.
- **DSP-05:** an expired or superseded fencing token cannot perform execution mutations.
- **DSP-06:** renewal cannot extend a lease beyond the session lifetime.
- **DSP-07:** rank scope and deterministic tie-breaking are documented and tested.
- **DSP-08:** contention, ineligibility, missing specification, blocked work, and permission denial are distinguishable results.

### Context

- **CTX-01:** context responses obey a server-enforced maximum size.
- **CTX-02:** every omitted or truncated section is declared.
- **CTX-03:** responses identify the state version from which they were built.
- **CTX-04:** imported external content is marked untrusted and delimited from workspace policy.
- **CTX-05:** omitted resources can be fetched individually when authorized.
- **CTX-06:** context includes relevant activity from a prior failed attempt.
- **CTX-07:** context generation is read-only and cannot unexpectedly trigger unbounded model work.

### Reporting

- **RPT-01:** execution reports require the active lease and fencing token.
- **RPT-02:** a report batch is validated before any mutation is committed.
- **RPT-03:** agent completion requires a resolution summary.
- **RPT-04:** agents can create discovered work with `discovered_from` provenance.
- **RPT-05:** requesting more specification creates a `needs_spec` hold and makes the reason visible to humans.
- **RPT-06:** duplicate resolution creates a validated `duplicate_of` relationship.
- **RPT-07:** supported retries do not create duplicate effects.

### Human controls

- **HUM-01:** humans can see which session currently leases an issue.
- **HUM-02:** humans can see lease health, heartbeat, and expiry.
- **HUM-03:** authorized humans can revoke a session.
- **HUM-04:** authorized humans can rank, hold, clarify, assign, release, and manually recover issues.
- **HUM-05:** approval requests display their exact proposed mutations, target version, and expiry.
- **HUM-06:** the UI distinguishes assignment from execution ownership.
- **HUM-07:** excluded work has an understandable reason for authorized human viewers.

### Identity and security

- **SEC-01:** every agent action is attributed to a role and session.
- **SEC-02:** role and session credentials can be independently revoked.
- **SEC-03:** every object access is workspace-isolated.
- **SEC-04:** retries cannot create duplicate effects for supported operations.
- **SEC-05:** external content cannot alter workspace policy or agent permissions.
- **SEC-06:** secrets do not appear in context or audit payloads.

## 10. Agent contract

The pilot exposes four primary intentions. The transport may be REST, CLI, MCP, or a thin combination, but the semantics must remain the same.

### `ready`

Returns eligible work, a snapshot cursor, stable pagination, rank, and enough exclusion information for an authorized caller to understand why relevant work was omitted.

### `claim`

Accepts an issue, requested TTL, and idempotency key. It re-evaluates eligibility, creates the exclusive lease, and returns `lease_id`, `fencing_token`, `expires_at`, and renewal policy.

### `context`

Accepts an issue and bounded budget. It returns issue identity, dispatch state, title, body excerpt, labels, fields, blockers, parent or discovery lineage, selected recent activity, relevant external state, and prior-attempt information, subject to authorization and size limits.

Every section includes provenance and trust class. `workspace_policy`, `workspace_content`, `external_untrusted`, and `agent_generated` must remain distinguishable.

### `report`

Accepts a bounded batch of progress and outcome operations. Initially supported operations are:

- append a progress or result comment;
- change lifecycle status within the caller's capability grant;
- release or complete the claim;
- set permitted fields;
- create discovered work with provenance;
- add a blocker;
- request more specification;
- close with a resolution summary;
- close as a duplicate with `duplicate_of`.

The server validates the complete batch before applying it unless a specific operation documents partial results.

## 11. Human control surface

The pilot UI consists of five focused surfaces:

1. **Triage:** new, needs-spec, held, and approval-waiting issues.
2. **Queue:** ranked list or board over the pilot queue, with rank controls.
3. **Issue detail:** body, relationships, dispatch state, active lease, activity, and external links.
4. **Approval queue:** exact proposed mutation, diff, expiry, approve, and deny.
5. **Agent roster:** roles, capabilities, active sessions, heartbeats, leases, and revoke controls.

The UI must show pending, confirmed, and rejected authoritative actions distinctly. It must expose a clear takeover action for authorized humans, including the reason, interrupted session, new owner, and recovery state. A human action that conflicts with active execution must explain the conflict and offer only supported recovery choices.

## 11.1 GitHub integration boundary

The pilot treats GitHub as an issue-level source and link target. It performs an initial issue import, then listens only to the repository `issues` webhook actions `opened`, `edited`, `closed`, `reopened`, `transferred`, and `deleted`. These events update the linked external-issue record and activity; they do not automatically change Riichi dispatch state or complete a Riichi issue.

The integration ignores pull requests returned by GitHub's issue endpoints, since GitHub represents pull requests as issues in those endpoints. Pull requests, checks, reviews, branches, and commits remain outside the pilot loop.

Riichi comments are a separate native activity stream. GitHub comments remain in GitHub, and the pilot does not subscribe to `issue_comment`. No GitHub event may automatically complete or mutate a Riichi issue without the documented validation and approval rules.

## 12. Acceptance scenarios

These scenarios are release gates for the pilot implementation.

1. **Concurrent claim:** given one ready issue, when 20 sessions claim it concurrently, exactly one receives a lease and the other 19 receive a contention or ineligibility result.
2. **Stale report:** given session A's lease has expired and session B has claimed the issue, when A submits a completion report with its old fencing token, authoritative issue state is unchanged.
3. **Blocked issue:** given an unresolved incoming `blocks` edge, when an authorized agent calls `ready`, the issue is excluded and the blocker is available as an exclusion reason.
4. **Needs specification:** given an agent reports insufficient specification, when the report is accepted, Riichi creates a visible `needs_spec` hold and the issue no longer appears in `ready`.
5. **Bounded context:** given a context request exceeds the configured budget, the response stays within the maximum and declares every omitted or truncated section.
6. **Untrusted import:** given an imported review comment tells the agent to ignore workspace policy, the context marks the text as external and untrusted, and no permission changes occur.
7. **Approval revalidation:** given an approval request targets issue version 4 and the issue is now version 6, when a human approves it, Riichi revalidates or supersedes the request instead of blindly applying stale intent.
8. **Retry safety:** given a create or report request is retried with the same idempotency key and body, only one effect is created and the original result is replayed.
9. **Session revocation:** given a session is revoked, subsequent renewal and report attempts fail, and its active leases become eligible for the documented recovery path.
10. **Workspace isolation:** given valid identifiers from two workspaces, a caller in workspace A cannot read or mutate workspace B's issues, relationships, context, leases, or audit records.
11. **Human recovery:** given an active lease is revoked by an authorized human, the issue's state and recovery path are visible to the next agent and to the original session.
12. **Prior attempt context:** given a previous session left a progress report before expiry, a new session's context includes that report with its original role, session, and timestamp.
13. **Takeover interruption:** given a human takes over an active lease, the old session receives an interruption signal, the new owner and recovery checklist are visible, and later mutations from the old session cannot change authoritative state.
14. **Collaborator recovery review:** given a collaborator has `approval_required` recovery-review capability, the collaborator can inspect quarantined attempt data and propose a recovery action, but cannot apply it without approval.
15. **GitHub issue boundary:** given GitHub sends an allowed `issues` event, Riichi updates the external-issue record without mirroring comments, importing pull requests, or changing dispatch state automatically.

## 13. Instrumentation and success measures

The pilot must establish a baseline during an observation week before judging improvement. Capture the same measures in the team's current workflow where possible.

### Primary product measures

- time from issue becoming executable to first successful claim;
- duplicate or conflicting agent attempts per 10 completed issues;
- human coordination minutes per completed issue;
- percentage of expired attempts that are successfully resumed;
- percentage of routine dispatch decisions performed in Riichi.

### Secondary product measures

- time spent assembling context before work begins;
- percentage of attempts that request more specification;
- human interventions per agent attempt;
- claim contention rate;
- time from lease expiry to safe reassignment;
- weekly active pilot workspaces and repeat use across consecutive weeks;
- discovered-work volume and duplicate rate;
- approval-request rate and approval turnaround time.

### Correctness invariants

These are release gates, not experiments:

- zero accepted stale-lease mutations;
- zero legitimate double claims;
- zero cross-workspace access in the authorization suite;
- zero duplicate effects in supported retry scenarios;
- 100% attribution of material agent mutations to role and session;
- no secret leakage in context, audit, or ordinary logs.

### Proposed pilot success rule

Confirm the exact thresholds with design partners before the observation week. The working rule is:

- at least five active pilot teams;
- at least four teams use Riichi in two consecutive weeks;
- the median of the primary measures improves by at least 30% against each team's baseline, or teams show a clear equivalent reduction in coordination burden;
- no correctness invariant fails in production pilot use;
- at least two teams state that Riichi replaces a routine coordination step rather than merely supplementing their existing tracker;
- the team can name the top three product changes required for the next PRD.

If adoption is weak but the correctness gates pass, treat that as product evidence, not as a reason to expand scope automatically.

## 14. Rollout plan

### Stage 0: internal dogfood

Use synthetic contention, lease expiry, delayed reports, retry, revocation, and cross-workspace tests. Run the workflow with a minimal internal console before inviting external teams.

**Exit criteria:** all acceptance scenarios pass; metrics are emitted and inspectable; the recovery path is documented.

### Stage 1: two design partners

Run manually monitored sessions with two teams. Observe baseline workflow, refine terminology, and resolve product decisions that block safe use.

**Exit criteria:** both teams complete repeated real work through the queue; neither requires database intervention for routine dispatch; the team has evidence about context usefulness and recovery.

### Stage 2: five-team pilot

Instrument normal use, conduct weekly interviews, and review correctness incidents immediately. Keep the cohort small enough to inspect individual failures.

**Exit criteria:** the proposed pilot success rule is met or the evidence clearly supports a revised wedge decision.

### Stage 3: limited beta decision

Only after lease safety, tenant isolation, retry safety, recovery, and repeat-use criteria pass, decide whether to build the complete MVP PRD.

## 15. Risks and mitigations

| Risk | Mitigation |
| --- | --- |
| Teams use Riichi only as a read-only dashboard | Require routine claim and report flow in the pilot; measure replacement of coordination steps. |
| Lease correctness looks good in demos but fails under contention | Run concurrent claims, delayed reports, expiry, revocation, and retry tests before external use. |
| Context is too large, stale, or unhelpful | Enforce a budget, expose provenance and omissions, and measure setup time and clarification requests. |
| Humans cannot understand why work is unavailable | Return stable exclusion reasons and show holds, blockers, claims, and permissions in the UI. |
| Imported content becomes an instruction channel | Delimit and label external content as untrusted; never let it change policy or permissions. |
| Pilot scope turns into a tracker replacement project | Keep the non-goals visible and defer workflow breadth, reporting, and mirroring. |
| Failed sessions leave unclear ownership | Expire leases automatically, show lease health, and include prior attempt activity in context. |
| Credentials or audit records leak sensitive data | Scope and revoke credentials, redact audit summaries, filter logs and context, and test the boundary. |

## 16. Decisions recorded from the initial product pass

These decisions are now the pilot defaults:

1. Claiming moves an issue from `todo` to `in_progress`.
2. A human can forcibly take over an active lease.
3. A lease has one fenced owner and may have explicitly delegated collaborators with capabilities up to `approval_required`; `never` capabilities cannot be delegated.
4. Takeover interrupts the old session, invalidates its fencing token, rejects later mutations from it, and creates a recovery checklist.
5. Collaborators with explicit recovery-review capability can inspect quarantined results; approval-required reviewers can propose recovery but cannot apply it without approval.
6. A canceled blocker stops blocking its targets in the same way as a completed blocker. Other unresolved blockers still apply.
7. `ready` returns an understandable exclusion reason to authorized callers without exposing unnecessary internal detail.
8. The approval defaults are the capability policy in section 7.5.
9. The initial lease duration, renewal cadence, and session lifetime are the provisional values in section 7.4.
10. The pilot GitHub boundary is issue-level integration with the selected `issues` events and a separate Riichi-native comments stream.
11. Human owners and admins receive recovery-review access by default. Ordinary team members can see that quarantined data exists but cannot read its payload. Agent collaborators receive `recovery_review` only through an explicit grant, and a revoked lease owner receives no automatic access.
12. The recovery checklist offers `reopen_for_dispatch` and `complete_with_summary` as one-click proposals. Continuing investigation leaves the checklist open; quarantined payloads are never replayed as an untyped recovery action.
13. The pilot includes approval-gated GitHub issue creation with repository authorization, idempotency, audit attribution, and a durable Riichi-to-GitHub link.

## 16.1 Recovery and GitHub decisions

These choices are recorded above. Expanding recovery visibility to ordinary team members, adding generic quarantined-payload replay, or permitting automatic agent-created GitHub issues requires a new product decision rather than an implicit capability change.

## 17. Technology boundary for the next discussion

The pilot requires these properties, regardless of implementation choices:

- server-authoritative arbitration for claims, approvals, permissions, idempotency, and integration side effects;
- transactional enforcement of exclusive leases and fencing;
- durable attribution and audit records;
- bounded, provenance-aware context construction;
- workspace and principal isolation;
- deterministic, inspectable recovery from session expiry;
- a transport that can expose the four intentions to real agent sessions;
- an observable path for notifications and integrations.

The pilot does not choose among databases, frameworks, hosting models, replication libraries, event-sourcing designs, or GitHub integration architectures. Those belong in technical RFCs after the product boundary and pilot evidence are agreed.

## 18. Definition of pilot done

The pilot is ready to invite external teams when:

- the four intentions work end to end for a real repository task;
- the human control surface supports triage, queue control, issue inspection, approvals, and session recovery;
- all correctness invariants and acceptance scenarios pass;
- baseline and pilot metrics are recorded with workspace and session attribution;
- the team has documented the open product decisions and their owners;
- an operator can diagnose contention, expiry, rejection, and recovery without direct database access;
- the pilot runbook covers credentials, incidents, data handling, and rollback.

The pilot is complete when the team can make a go, revise, or stop decision about the shared-dispatch wedge using observed evidence.
