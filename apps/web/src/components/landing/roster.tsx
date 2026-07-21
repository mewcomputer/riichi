import { cn } from "@/lib/utils";
import { people, roster } from "./_/data";
import { Reveal } from "./_/reveal";

export function Roster() {
  return (
    <section id="roster" className="relative py-20 md:py-28 border-t border-border">
      <div className="mx-auto max-w-[1440px] px-6 md:px-10">
        <Reveal>
          <div className="font-mono text-[10.5px] uppercase tracking-[0.16em] text-muted-foreground/60">
            Roster
          </div>
          <h2 className="mt-3 text-[clamp(36px,5.5vw,68px)] leading-[1.02] tracking-[-0.025em] font-medium max-w-[20ch]">
            <span className="text-foreground">Stay ahead of</span>
            <br className="hidden sm:block" />
            <span className="text-muted-foreground">failure.</span>
          </h2>
          <p className="mt-5 max-w-[58ch] text-[14.5px] leading-[1.55] text-muted-foreground">
            Grant only what an agent needs, see every active session, and let
            Riichi handle the takeover when a run goes sideways. No postmortem
            required.
          </p>
        </Reveal>

        <div className="mt-14 md:mt-20 grid grid-cols-12 gap-6 md:gap-8">
          {/* Roster list */}
          <Reveal as="div" delay={80} className="col-span-12 lg:col-span-6">
            <div className="rounded-[14px] border border-border bg-card overflow-hidden">
              <div className="flex items-center justify-between border-b border-border px-5 md:px-6 py-3.5">
                <div className="font-mono text-[10.5px] uppercase tracking-[0.16em] text-muted-foreground/70">
                  {roster.teamName} · agent roster
                </div>
                <div className="font-mono text-[11px] text-muted-foreground/70">
                  <span className="tabular-nums text-foreground">{roster.activeSessions}</span>
                  <span className="text-muted-foreground/50"> / </span>
                  <span className="tabular-nums">{roster.totalSessions}</span> active
                </div>
              </div>
              <ul>
                {roster.roles.map((role) => (
                  <RoleRow key={role.id} role={role} />
                ))}
              </ul>
            </div>
          </Reveal>

          {/* Recovery event */}
          <Reveal as="aside" delay={140} className="col-span-12 lg:col-span-6">
            <RecoveryCard />
          </Reveal>
        </div>
      </div>
    </section>
  );
}

function RoleRow({
  role,
}: {
  role: (typeof roster.roles)[number];
}) {
  const initials = role.name
    .split("-")
    .map((s) => s[0])
    .join("")
    .slice(0, 2)
    .toUpperCase();
  const resolvedBy = people.maya;

  return (
    <li className="border-b border-border last:border-b-0 px-5 md:px-6 py-5 grid grid-cols-12 gap-3 items-start">
      <div className="col-span-12 md:col-span-4 flex items-center gap-2.5">
        <span
          className={cn(
            "size-7 rounded-full grid place-items-center font-mono text-[10px] font-medium ring-1 ring-card",
            role.activeSessionCount > 0
              ? "bg-primary text-background"
              : "bg-muted-foreground/40 text-background",
          )}
        >
          {initials}
        </span>
        <div>
          <div className="font-mono text-[13px] text-foreground">{role.name}</div>
          <div className="font-mono text-[10.5px] text-muted-foreground/60 tabular-nums">
            last dispatched {role.lastDispatched}
          </div>
        </div>
      </div>

      <div className="col-span-12 md:col-span-5 flex flex-wrap items-center gap-1">
        {role.capabilities.map((cap) => (
          <span
            key={cap}
            className="rounded border border-border bg-muted/40 px-1.5 py-0.5 font-mono text-[10.5px] text-muted-foreground"
          >
            {cap}
          </span>
        ))}
      </div>

      <div className="col-span-12 md:col-span-3 md:text-right">
        {role.activeSessionCount > 0 ? (
          <span className="inline-flex items-center gap-1.5 font-mono text-[11px] text-foreground">
            <span className="size-1.5 rounded-full bg-primary" />
            <span className="tabular-nums">{role.activeSessionCount}</span>
            <span className="text-muted-foreground/60">active</span>
          </span>
        ) : (
          <span className="font-mono text-[11px] text-muted-foreground/50">
            idle
          </span>
        )}
      </div>
    </li>
  );
}

function RecoveryCard() {
  const r = roster.recovery;
  const resolver = people[r.resolvedBy];

  return (
    <div className="rounded-[14px] border border-border bg-card p-5 md:p-6 h-full flex flex-col">
      <div className="flex items-center justify-between mb-3">
        <div className="font-mono text-[10.5px] uppercase tracking-[0.16em] text-muted-foreground/70">
          Recovery · {r.sessionId}
        </div>
        <span className="inline-flex items-center gap-1.5 rounded-full border border-border bg-muted/40 px-2 py-0.5 font-mono text-[10px] uppercase tracking-[0.14em] text-muted-foreground/80">
          <span className="size-1.5 rounded-full bg-primary" />
          resolved
        </span>
      </div>

      <div className="text-[12px] font-mono text-muted-foreground/70 tabular-nums">
        {r.occurredAt}
      </div>
      <p className="mt-3 text-[14px] leading-[1.6] text-foreground">
        {r.summary}
      </p>

      <div className="mt-5 grid grid-cols-2 gap-x-4 gap-y-3 font-mono text-[11px]">
          <div>
          <div className="text-[9.5px] uppercase tracking-[0.16em] text-muted-foreground/60">
            Agent
          </div>
          <div className="mt-0.5 text-foreground">{r.agent}</div>
        </div>
        <div>
          <div className="text-[9.5px] uppercase tracking-[0.16em] text-muted-foreground/60">
            Quarantined
          </div>
          <div className="mt-0.5 text-primary tabular-nums">
            {r.quarantinedArtifacts} artifact
            {r.quarantinedArtifacts === 1 ? "" : "s"}
          </div>
        </div>
        <div className="col-span-2">
          <div className="text-[9.5px] uppercase tracking-[0.16em] text-muted-foreground/60">
            Resolved by
          </div>
          <div className="mt-0.5 flex items-center gap-2">
            <span
              className={cn(
                "size-5 rounded-full grid place-items-center font-mono text-[9px] font-medium",
                "ring-1 ring-card",
              )}
              style={{
                background: "var(--foreground)",
                color: "var(--background)",
              }}
            >
              {resolver.initials}
            </span>
            <span className="text-foreground">{resolver.name}</span>
          </div>
        </div>
      </div>

      <div className="mt-auto pt-5 flex items-center gap-2 font-mono text-[11px] text-muted-foreground/60">
        <span className="size-1.5 rounded-full bg-primary" />
        Token retired · next claim mints a fresh one.
      </div>
    </div>
  );
}
