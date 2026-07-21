import { useEffect, useState, type ReactNode } from "react";
import { cn } from "@/lib/utils";
import { activeLease, issues, team, type Issue, type IssueStatus } from "./data";
import { useReducedMotion } from "./reveal";

/**
 * Linear-style product screenshot for the hero. A single issue detail view
 * with a left sidebar and an active-agent panel on the right. Keeps the mock
 * based on the real product surface without the chrome of the old workbench.
 */
export function Screenshot({ className }: { className?: string }) {
  return (
    <div
      className={cn(
        "relative overflow-hidden rounded-[22px] border border-border bg-card",
        "shadow-[0_1px_0_rgba(255,255,255,0.03)_inset,0_40px_100px_-40px_rgba(0,0,0,0.5)]",
        className,
      )}
    >
      <TopChrome />
      <div className="grid grid-cols-12 min-h-[520px]">
        <Sidebar />
        <main className="col-span-12 md:col-span-9 lg:col-span-7 min-w-0 border-t md:border-t-0 border-border">
          <IssueDetail />
        </main>
        <aside className="hidden lg:block col-span-3 border-t lg:border-t-0 lg:border-l border-border bg-background">
          <AgentPanel />
        </aside>
      </div>
    </div>
  );
}

/* ---------- chrome ---------- */

function TopChrome() {
  return (
    <div className="flex h-11 items-center gap-3 border-b border-border bg-background px-4 md:px-5">
      <div className="flex items-center gap-1.5">
        <span className="size-2.5 rounded-full bg-muted-foreground/30" />
        <span className="size-2.5 rounded-full bg-muted-foreground/25" />
        <span className="size-2.5 rounded-full bg-muted-foreground/20" />
      </div>
      <div className="hidden sm:flex items-center gap-2 ml-3 font-mono text-[11.5px] text-muted-foreground/70">
        <span>{team.org.toLowerCase()}.riichi.app</span>
        <span className="text-muted-foreground/40">/</span>
        <span className="text-foreground">{team.name}</span>
        <span className="text-muted-foreground/40">/</span>
        <span className="text-foreground">{activeLease.issueKey}</span>
      </div>
    </div>
  );
}

/* ---------- sidebar ---------- */

function Sidebar() {
  return (
    <aside className="hidden md:block col-span-3 lg:col-span-2 border-r border-border bg-background p-4">
      <div className="font-mono text-[10.5px] uppercase tracking-[0.16em] text-muted-foreground/60">
        Workspace
      </div>
      <div className="mt-2 space-y-0.5">
        <SidebarItem label="Queue" count="6" />
        <SidebarItem label="My issues" count="2" />
        <SidebarItem label="Agents" count="3" active />
        <SidebarItem label="Decisions" muted soon />
      </div>

      <div className="mt-6 font-mono text-[10.5px] uppercase tracking-[0.16em] text-muted-foreground/60">
        Teams
      </div>
      <div className="mt-2 space-y-0.5">
        <SidebarItem label="Platform" dot />
        <SidebarItem label="Design" muted />
        <SidebarItem label="Growth" muted />
      </div>
    </aside>
  );
}

function SidebarItem({
  label,
  count,
  active,
  muted,
  soon,
  dot,
}: {
  label: string;
  count?: string;
  active?: boolean;
  muted?: boolean;
  soon?: boolean;
  dot?: boolean;
}) {
  return (
    <div
      className={cn(
        "flex items-center gap-2 rounded-md px-2 py-1.5 text-[13px]",
        active ? "bg-foreground text-background" : "hover:bg-muted",
      )}
    >
      {dot && <span className="size-1.5 rounded-full bg-primary" />}
      <span className={cn("truncate", !active && muted && "text-muted-foreground/70")}>{label}</span>
      {soon && (
        <span className="ml-1 rounded-sm bg-muted px-1 py-px font-mono text-[9px] uppercase tracking-[0.14em] text-muted-foreground/70">
          soon
        </span>
      )}
      {count && (
        <span
          className={cn(
            "ml-auto font-mono text-[11px] tabular-nums",
            active ? "opacity-80" : "text-muted-foreground/60",
          )}
        >
          {count}
        </span>
      )}
    </div>
  );
}

/* ---------- issue detail ---------- */

const issue = issues[0];

function IssueDetail() {
  return (
    <div className="flex h-full flex-col p-5 md:p-7">
      <div className="flex items-center gap-2 font-mono text-[11px] text-muted-foreground/70">
        <span>{team.name}</span>
        <span className="text-muted-foreground/40">·</span>
        <span>{team.cycle}</span>
      </div>

      <div className="mt-3 flex flex-wrap items-center gap-2">
        <span className="font-mono text-[12px] tabular-nums text-muted-foreground/70">
          {issue.displayKey}
        </span>
        <StatusBadge status={issue.status} />
        <ImportanceTag importance={issue.importance} />
      </div>

      <h2 className="mt-3 text-[20px] md:text-[24px] leading-tight tracking-[-0.015em] font-medium">
        {issue.title}
      </h2>

      <p className="mt-3 text-[14px] leading-[1.6] text-muted-foreground max-w-[62ch]">
        Refactor the enrollment state machine so device activation moves through a single transition
        table. Hold release until the legacy client path is confirmed and the new tests are green.
      </p>

      <div className="mt-6 flex items-center gap-3 border-y border-border py-3 font-mono text-[11px] text-muted-foreground/70">
        <span className="flex items-center gap-1.5">
          <span className="size-1.5 rounded-full bg-primary" />
          Leased to {activeLease.agent}
        </span>
        <span className="text-muted-foreground/40">·</span>
        <span>Token {activeLease.fencingToken}</span>
        <span className="text-muted-foreground/40">·</span>
        <span>Expires {activeLease.expiresAt}</span>
      </div>

      <div className="mt-6">
        <h3 className="font-mono text-[10.5px] uppercase tracking-[0.14em] text-muted-foreground/60">
          Activity
        </h3>
        <div className="mt-3 space-y-4">
          <ActivityItem
            actor="platform-builder"
            time="2 min ago"
            body="Reported progress: refactored transition table, 12 tests passing."
          />
          <ActivityItem
            actor="ren"
            time="44 min ago"
            body="Requested spec hold cleared after legacy path was documented."
          />
          <ActivityItem
            actor="riichi"
            time="1h ago"
            body="Lease claimed with fencing token 0x4a2f and capabilities granted."
          />
        </div>
      </div>
    </div>
  );
}

function ActivityItem({ actor, time, body }: { actor: string; time: string; body: string }) {
  return (
    <div className="flex gap-3">
      <Avatar tone={actor === "riichi" ? "b" : actor === "ren" ? "a" : "c"} small>
        {initials(actor)}
      </Avatar>
      <div>
        <div className="flex items-center gap-2 text-[13px]">
          <span className="font-medium">{actor}</span>
          <span className="font-mono text-[11px] text-muted-foreground/60">{time}</span>
        </div>
        <p className="mt-0.5 text-[13px] leading-relaxed text-muted-foreground">{body}</p>
      </div>
    </div>
  );
}

function initials(name: string) {
  const parts = name.split(/[-\s]+/);
  if (parts.length >= 2) return (parts[0][0] + parts[1][0]).toUpperCase();
  return name.slice(0, 2).toUpperCase();
}

function StatusBadge({ status }: { status: IssueStatus }) {
  const labels: Record<IssueStatus, string> = {
    triage: "Triage",
    todo: "Todo",
    in_progress: "In progress",
    review: "Review",
    done: "Done",
    canceled: "Canceled",
    blocked: "Blocked",
  };
  return (
    <span className="inline-flex items-center gap-1.5 rounded-full border border-border bg-muted/50 px-2 py-0.5 font-mono text-[10.5px] uppercase tracking-[0.12em] text-muted-foreground">
      <StatusDot status={status} />
      {labels[status]}
    </span>
  );
}

function StatusDot({ status }: { status: IssueStatus }) {
  const cls = "inline-block size-2 rounded-full shrink-0";
  if (status === "in_progress") return <span className={cn(cls, "bg-primary")} />;
  if (status === "review") return <span className={cn(cls, "bg-muted-foreground")} />;
  if (status === "done") return <span className={cn(cls, "bg-foreground")} />;
  if (status === "triage" || status === "blocked") return <span className={cn(cls, "bg-primary/70")} />;
  if (status === "canceled") return <span className={cn(cls, "bg-muted-foreground/30")} />;
  return <span className={cn(cls, "border border-muted-foreground/40")} />;
}

function ImportanceTag({ importance }: { importance: Issue["importance"] }) {
  const bars = importance === "urgent" ? 4 : importance === "high" ? 3 : importance === "med" ? 2 : 1;
  return (
    <span className="inline-flex items-center gap-1.5 font-mono text-[10.5px] text-muted-foreground/70">
      <span className="inline-flex items-end gap-px">
        {[1, 2, 3, 4].map((n) => (
          <span
            key={n}
            className="w-[2px] rounded-sm bg-current"
            style={{
              height: `${n * 2 + 1}px`,
              opacity: n <= bars ? 1 : 0.25,
            }}
          />
        ))}
      </span>
      <span className="uppercase tracking-[0.12em]">{importance}</span>
    </span>
  );
}

/* ---------- active agent panel ---------- */

function AgentPanel() {
  const lease = activeLease;
  return (
    <div className="flex h-full flex-col p-5">
      <div className="rounded-xl border border-border bg-muted/30 p-4">
        <div className="flex items-center gap-2">
          <Avatar tone="b">{initials(lease.agent)}</Avatar>
          <div>
            <div className="text-[13px] font-medium">{lease.agent}</div>
            <div className="font-mono text-[10px] uppercase tracking-[0.12em] text-muted-foreground/70">
              Active lease
            </div>
          </div>
        </div>

        <div className="mt-4 space-y-3 font-mono text-[11px]">
          <div className="flex justify-between">
            <span className="text-muted-foreground/60">Token</span>
            <span className="text-primary tabular-nums">{lease.fencingToken}</span>
          </div>
          <div className="flex justify-between">
            <span className="text-muted-foreground/60">Issued</span>
            <span className="tabular-nums">{lease.issuedAt}</span>
          </div>
          <div className="flex justify-between">
            <span className="text-muted-foreground/60">Expires</span>
            <span className="tabular-nums">{lease.expiresAt}</span>
          </div>
        </div>

        <div className="mt-4 border-t border-border pt-3">
          <div className="font-mono text-[10px] uppercase tracking-[0.12em] text-muted-foreground/60">
            Capabilities
          </div>
          <div className="mt-2 flex flex-wrap gap-1">
            {lease.capabilities.map((cap) => (
              <span
                key={cap}
                className="rounded border border-border bg-background px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground"
              >
                {cap}
              </span>
            ))}
          </div>
        </div>
      </div>

      <div className="mt-4 rounded-xl border border-border p-4">
        <div className="font-mono text-[10px] uppercase tracking-[0.12em] text-muted-foreground/60">
          Last report
        </div>
        <p className="mt-2 text-[12px] leading-[1.55] text-muted-foreground line-clamp-4">
          {lease.lastReport}
        </p>
        <div className="mt-3 flex items-center gap-4 border-t border-border pt-2 font-mono text-[11px]">
          {lease.artifacts.map((a) => (
            <span key={a.label} className="flex items-baseline gap-1">
              <span className="text-[9.5px] uppercase tracking-[0.14em] text-muted-foreground/60">
                {a.label}
              </span>
              <span className="tabular-nums text-foreground">{a.value}</span>
            </span>
          ))}
        </div>
      </div>

      <div className="mt-auto pt-5">
        <CommandCue />
      </div>
    </div>
  );
}

function CommandCue() {
  const reduced = useReducedMotion();
  const final = "Tell platform-builder what to do next...";
  const [text, setText] = useState(reduced ? final : "");

  useEffect(() => {
    if (reduced) {
      setText(final);
      return;
    }
    let i = 0;
    setText("");
    const id = setInterval(() => {
      i += 1;
      setText(final.slice(0, i));
      if (i >= final.length) {
        clearInterval(id);
      }
    }, 45);
    return () => clearInterval(id);
  }, [reduced]);

  return (
    <div className="rounded-full border border-border bg-muted/40 px-4 py-2.5 font-mono text-[12px] text-muted-foreground/70">
      <span className="text-foreground">{text}</span>
      {!reduced && text.length < final.length && (
        <span className="caret ml-px inline-block h-[15px] w-[7px] bg-primary align-middle" />
      )}
    </div>
  );
}

function Avatar({
  children,
  tone,
  small,
}: {
  children: ReactNode;
  tone: "a" | "b" | "c" | "d";
  small?: boolean;
}) {
  const map = {
    a: { bg: "var(--foreground)", fg: "var(--background)" },
    b: { bg: "var(--primary)", fg: "var(--background)" },
    c: { bg: "var(--muted-foreground)", fg: "var(--background)" },
    d: { bg: "var(--muted)", fg: "var(--foreground)" },
  } as const;
  const size = small ? "size-6 text-[10px]" : "size-7 text-[11px]";
  return (
    <span
      className={cn(
        size,
        "rounded-full grid place-items-center font-mono font-medium ring-1 ring-card shrink-0",
      )}
      style={{ background: map[tone].bg, color: map[tone].fg }}
    >
      {children}
    </span>
  );
}


