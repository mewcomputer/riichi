/**
 * Single narrative thread for the landing page.
 *
 * Cast, issues, and agents are shared across every section so the hero
 * screenshot, dispatch lifecycle, and roster deep-dive describe one
 * coherent shift on the Platform team.
 *
 * Language matches the actual product surface (queue.ts, agents.tsx):
 *   - Queue state: ready | attention | held
 *   - Issue status: triage | todo | in_progress | review | done | canceled | blocked
 *   - Importance: low | med | high | urgent
 *   - Reason strings: "Ready for dispatch", "Leased to an agent", etc.
 *   - Agent capabilities: comment, request_spec, discover, complete, release,
 *     doc.read, doc.apply_edit
 */

export type Tone = "a" | "b" | "c" | "d";

export interface Person {
  initials: string;
  name: string;
  tone: Tone;
}

export const people = {
  maya: { initials: "MK", name: "Maya K.", tone: "a" },
  ren: { initials: "RN", name: "Ren N.", tone: "b" },
  jules: { initials: "JS", name: "Jules S.", tone: "c" },
  alma: { initials: "AL", name: "Alma L.", tone: "d" },
} as const satisfies Record<string, Person>;

export type PersonKey = keyof typeof people;

export type QueueState = "ready" | "attention" | "held";
export type IssueStatus =
  | "triage"
  | "todo"
  | "in_progress"
  | "review"
  | "done"
  | "canceled"
  | "blocked";
export type Importance = "low" | "med" | "high" | "urgent";

export interface Issue {
  displayKey: string;
  title: string;
  status: IssueStatus;
  importance: Importance;
  state: QueueState;
  reason: string;
  age: string;
  activeLease?: LeaseRef;
  owner?: PersonKey;
  team: string;
}

export interface LeaseRef {
  agentId: string;
  agentName: string;
  fencingToken: string;
  issuedAt: string;
  expiresAt: string;
}

export const team = {
  org: "Northwind",
  name: "Platform",
  key: "PLT",
  cycle: "Cycle 34",
  cycleWindow: "Mar 3 — Mar 17",
  cycleDay: "Day 9 / 14",
} as const;

export const issues: Issue[] = [
  {
    displayKey: "ENG-218",
    title: "Unify device enrollment state machine",
    status: "in_progress",
    importance: "high",
    state: "attention",
    reason: "Leased to platform-builder",
    age: "2h 14m",
    owner: "maya",
    activeLease: {
      agentId: "platform-builder",
      agentName: "platform-builder",
      fencingToken: "0x4a2f",
      issuedAt: "09:14",
      expiresAt: "11:14",
    },
    team: "Platform",
  },
  {
    displayKey: "ENG-224",
    title: "Retry policy for offline activation",
    status: "todo",
    importance: "high",
    state: "ready",
    reason: "Ready for dispatch",
    age: "6h 02m",
    owner: "ren",
    team: "Platform",
  },
  {
    displayKey: "ENG-237",
    title: "Document activation retry policy",
    status: "todo",
    importance: "low",
    state: "held",
    reason: "1 active hold · awaiting approval",
    age: "1d 3h",
    team: "Platform",
  },
  {
    displayKey: "ENG-242",
    title: "Backfill legacy comment threads into thread graph",
    status: "todo",
    importance: "med",
    state: "ready",
    reason: "Ready for dispatch",
    age: "3h 48m",
    owner: "ren",
    team: "Platform",
  },
  {
    displayKey: "ENG-248",
    title: "Latency budget: keep p95 under 120ms on write",
    status: "in_progress",
    importance: "urgent",
    state: "attention",
    reason: "Leased to platform-builder",
    age: "44m",
    owner: "jules",
    activeLease: {
      agentId: "platform-builder",
      agentName: "platform-builder",
      fencingToken: "0x4a30",
      issuedAt: "10:32",
      expiresAt: "12:32",
    },
    team: "Platform",
  },
  {
    displayKey: "ENG-231",
    title: "Revise setup failure taxonomy",
    status: "triage",
    importance: "med",
    state: "held",
    reason: "Spec needed",
    age: "2d",
    team: "Platform",
  },
];

export const activeLease = {
  issueKey: "ENG-218",
  issueTitle: "Unify device enrollment state machine",
  agent: "platform-builder",
  capabilities: ["discover", "complete", "doc.apply_edit", "release"] as const,
  fencingToken: "0x4a2f",
  previousToken: "0x4a2e",
  issuedAt: "09:14 UTC",
  expiresAt: "11:14 UTC",
  ttlMinutes: 120,
  lastReport:
    "Refactored enrollment state machine into a single transition table. Twelve new tests passing. Holding release until legacy client path is confirmed.",
  reportedAt: "10:42 UTC",
  artifacts: [
    { label: "Diff", value: "+312 / −148", href: "#product" },
    { label: "Tests", value: "12 passing", href: "#product" },
    { label: "Files", value: "8 touched", href: "#product" },
  ],
} as const;

export const roster = {
  teamName: "Platform",
  totalSessions: 4,
  activeSessions: 3,
  roles: [
    {
      id: "role_scout",
      name: "platform-scout",
      capabilities: ["discover", "comment", "doc.read"] as const,
      activeSessionCount: 1,
      lastDispatched: "1h ago",
    },
    {
      id: "role_builder",
      name: "platform-builder",
      capabilities: ["discover", "complete", "doc.apply_edit", "release"] as const,
      activeSessionCount: 2,
      lastDispatched: "44m ago",
    },
    {
      id: "role_scribe",
      name: "docs-scribe",
      capabilities: ["doc.read", "doc.apply_edit"] as const,
      activeSessionCount: 0,
      lastDispatched: "yesterday",
    },
  ],
  recovery: {
    agent: "platform-builder",
    sessionId: "sess_8e21",
    occurredAt: "Mar 9, 14:22",
    summary:
      "Takeover triggered after three consecutive report timeouts. Quarantined partial diff for review. Lease released, fencing token retired.",
    quarantinedArtifacts: 1,
    resolvedBy: "maya" as PersonKey,
  },
} as const;

export const dispatchLifecycle = {
  steps: [
    {
      n: "01",
      label: "Available",
      title: "Issue becomes dispatchable",
      body: "Status moves to todo with no active holds or unresolved blockers. Riichi marks it ready and surfaces it to the agents you've granted capabilities.",
      detail: "Status: todo · Holds: 0 · Blockers: 0",
    },
    {
      n: "02",
      label: "Claimed",
      title: "An agent takes the lease",
      body: "The first eligible agent claims the issue and receives a monotonic fencing token. The token is the only proof that lets later writes land.",
      detail: "Lease · Token 0x4a2f · TTL 120m",
    },
    {
      n: "03",
      label: "Reporting",
      title: "Progress arrives under fence",
      body: "Reports carry the token. Anything stale is rejected at the boundary, so a recovered agent can't mutate work that has moved on without them.",
      detail: "Reports verified against token",
    },
    {
      n: "04",
      label: "Resolved",
      title: "Release, revoke, or complete",
      body: "The agent completes the lease, a human revokes it, or the TTL lapses. The token retires. The next claim mints a new one.",
      detail: "Token retired · Next claim: 0x4a30",
    },
  ],
} as const;

export const principles = [
  {
    n: "01",
    title: "You control what agents can touch",
    body: "Every agent role carries an explicit grant — discover, comment, complete, release. There is no implicit privilege, and no path around the boundary.",
  },
  {
    n: "02",
    title: "Agents can't clobber each other's work",
    body: "Claims mint a monotonically increasing token. Every mutation carries it. Stale writes are rejected at the boundary, so a recovered agent cannot overwrite work that moved on.",
  },
  {
    n: "03",
    title: "Failed runs don't become mysteries",
    body: "Every claim, report, and release is recorded. When an agent fails, a takeover mints a recovery checklist and quarantines the partial work for review — nothing is silent.",
  },
] as const;

export const comparison = {
  columns: ["Bare LLM scripts", "Riichi dispatch"] as const,
  rows: [
    {
      label: "Capability model",
      left: "Whatever the model can reach",
      right: "Bounded per role, revocable per session",
    },
    {
      label: "Concurrency",
      left: "Lost updates, race conditions",
      right: "Stale writes are rejected at the boundary",
    },
    {
      label: "Failure handling",
      left: "You find out the next morning",
      right: "Takeover and quarantine happen automatically",
    },
    {
      label: "Audit trail",
      left: "Hunt through logs, if you remembered",
      right: "Every action is recorded and linked to the work",
    },
    {
      label: "Human override",
      left: "Drop everything to fix a runaway agent",
      right: "Pause, approve, or revoke without stopping the world",
    },
  ],
} as const;

export const faqs = [
  {
    q: "What stops an agent from doing something destructive?",
    a: "Capabilities. Every role carries an explicit grant (discover, comment, complete, doc.apply_edit, release, and so on). Sessions inherit the role's bounds. There is no implicit privilege, and any role or session can be revoked instantly from the roster.",
  },
  {
    q: "How do you handle two agents racing on the same issue?",
    a: "Fencing tokens. The first claim mints a monotonically increasing token; every subsequent write carries it. If a second agent — or a recovered first agent — tries to mutate the issue with a stale token, the write is rejected at the boundary. No last-write-wins, no race conditions.",
  },
  {
    q: "What happens when an agent fails mid-lease?",
    a: "After three consecutive report timeouts (configurable), Riichi triggers a takeover. The lease is released, the fencing token is retired, and a recovery checklist is created. Any partial diff is quarantined for review before it can land.",
  },
  {
    q: "Can a human intervene without stopping everything?",
    a: "Yes. Holds (needs_spec, awaiting_approval, scheduled, integration, manual) block dispatch on a specific issue without touching the rest of the queue. Approvals gate risky operations behind a human sign-off. Revocations pull a session or role immediately.",
  },
  {
    q: "Is there a CLI?",
    a: "Yes. The same protocol the web app uses is available as a first-party CLI. Our own engineering team runs agents through it daily; it is not a marketing artifact. Dispatch, report, hold, approve, and revoke are all callable from scripts and CI.",
  },
  {
    q: "Where does the work actually happen?",
    a: "Wherever you run the agent. Riichi issues the lease, the fencing token, and the capability grant. The compute is yours — your laptop, a CI runner, a dedicated cluster. Riichi observes the work, verifies the writes, and holds the audit trail.",
  },
] as const;

export const buildVersion = "v2026.03.14";
export const buildHash = "b78e21f";
