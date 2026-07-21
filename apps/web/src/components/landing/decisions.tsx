import { Reveal } from "./_/reveal";

const decisions = [
  {
    title: "Use UUID v7 for display IDs",
    by: "maya",
    when: "Mar 3",
    status: "accepted",
  },
  {
    title: "Retry policy: exponential backoff with jitter",
    by: "ren",
    when: "Mar 5",
    status: "accepted",
  },
  {
    title: "Scope cut: skip legacy import path",
    by: "platform-builder",
    when: "Mar 7",
    status: "proposed",
  },
] as const;

export function DecisionsTeaser() {
  return (
    <section
      id="decisions"
      className="relative py-20 md:py-28 border-t border-border overflow-hidden"
    >
      <div className="mx-auto max-w-[1440px] px-6 md:px-10">
        <div className="grid grid-cols-12 gap-6 md:gap-10 items-center">
          <Reveal as="div" className="col-span-12 lg:col-span-5">
            <div className="font-mono text-[10.5px] uppercase tracking-[0.16em] text-muted-foreground/60">
              Decisions
            </div>
            <h2 className="mt-3 text-[clamp(36px,5.5vw,68px)] leading-[1.02] tracking-[-0.025em] font-medium max-w-[16ch]">
              <span className="text-foreground">Keep the why.</span>
              <br className="hidden sm:block" />
              <span className="text-muted-foreground">Not just the what.</span>
            </h2>
            <p className="mt-6 text-[15px] leading-[1.55] max-w-[42ch] text-muted-foreground">
              Today, Riichi dispatches issues with clear ownership and a full audit trail.
              Tomorrow, every decision that shaped the work lives in the same thread —
              searchable, reversible, and readable by humans and agents.
            </p>
            <div className="mt-6 inline-flex items-center gap-2 rounded-full border border-border bg-muted/40 px-3 py-1.5 font-mono text-[11px] text-muted-foreground/80">
              <span className="size-1.5 rounded-full bg-primary" />
              On the roadmap
            </div>
          </Reveal>

          <Reveal as="div" delay={120} className="col-span-12 lg:col-span-7">
            <div className="relative rounded-[14px] border border-border bg-card p-6 md:p-8">
              <div className="flex items-center justify-between mb-6">
                <div className="font-mono text-[10.5px] uppercase tracking-[0.16em] text-muted-foreground/60">
                  ENG-224 · decision thread
                </div>
                <div className="rounded-full border border-border bg-muted/40 px-2 py-1 font-mono text-[10px] text-muted-foreground/70">
                  3 decisions
                </div>
              </div>

              <div className="relative">
                <div
                  className="absolute left-[15px] top-3 bottom-3 w-px bg-border"
                  aria-hidden
                />
                <ul className="space-y-5">
                  {decisions.map((d) => (
                    <li key={d.title} className="relative flex gap-4 pl-1">
                      <span
                        className="relative z-10 mt-1.5 size-3 rounded-full shrink-0"
                        style={{
                          background:
                            d.status === "accepted"
                              ? "var(--primary)"
                              : "var(--muted-foreground)",
                        }}
                      />
                      <div className="flex-1 min-w-0">
                        <div className="flex flex-wrap items-baseline gap-x-2 gap-y-1">
                          <span className="text-[14px] font-medium">{d.title}</span>
                          <span
                            className="rounded-full border border-border px-1.5 py-px font-mono text-[9px] uppercase tracking-[0.12em] text-muted-foreground/70"
                          >
                            {d.status}
                          </span>
                        </div>
                        <div className="mt-1 flex items-center gap-2 font-mono text-[11px] text-muted-foreground/60">
                          <span className="tabular-nums">{d.when}</span>
                          <span aria-hidden>·</span>
                          <span>{d.by}</span>
                        </div>
                      </div>
                    </li>
                  ))}
                </ul>
              </div>

              <div className="mt-6 border-t border-border pt-4 font-mono text-[11px] text-muted-foreground/60">
                <span className="text-primary">+</span> Start a decision from a report, a comment, or a hold
              </div>
            </div>
          </Reveal>
        </div>
      </div>
    </section>
  );
}
