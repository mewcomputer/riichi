import { cn } from "@/lib/utils";
import { comparison, principles } from "./_/data";
import { Reveal, RevealItem } from "./_/reveal";

export function Principles() {
  return (
    <section
      id="principles"
      className="relative py-20 md:py-28"
    >
      <div className="mx-auto max-w-[1440px] px-6 md:px-10">
        <Reveal>
          <div className="font-mono text-[10.5px] uppercase tracking-[0.16em] text-muted-foreground/60">
            Principles
          </div>
          <h2 className="mt-3 text-[clamp(36px,5.5vw,68px)] leading-[1.02] tracking-[-0.025em] font-medium max-w-[20ch]">
            <span className="text-foreground">Trust every</span>
            <br className="hidden sm:block" />
            <span className="text-muted-foreground">handoff.</span>
          </h2>
          <p className="mt-5 max-w-[58ch] text-[14.5px] leading-[1.55] text-muted-foreground">
            Agents get only the capabilities you grant. Every write is fenced.
            When something fails, you get a recovery checklist — not a mystery
            to debug next week.
          </p>
        </Reveal>

        <div className="mt-14 md:mt-20">
          {principles.map((p, i) => (
            <RevealItem
              key={p.n}
              delay={i * 60}
              className="border-t border-border py-7 md:py-9"
            >
              <article className="grid grid-cols-12 gap-4 md:gap-8 items-baseline">
                <div className="col-span-2 md:col-span-1 font-mono text-[11px] tabular-nums text-muted-foreground/60">
                  {p.n}
                </div>
                <h3 className="col-span-10 md:col-span-4 text-[22px] md:text-[28px] tracking-[-0.015em] font-medium">
                  {p.title}
                </h3>
                <p className="col-span-12 md:col-span-6 md:col-start-7 text-[14.5px] leading-[1.6] text-muted-foreground">
                  {p.body}
                </p>
              </article>
            </RevealItem>
          ))}
        </div>

        <Reveal as="div" delay={120} className="mt-20 md:mt-28">
          <div className="font-mono text-[10.5px] uppercase tracking-[0.16em] mb-5 text-muted-foreground/60">
            Comparison
          </div>
          <div className="overflow-hidden rounded-[14px] border border-border">
            <div className="grid grid-cols-12 bg-muted/40 px-5 md:px-7 py-3 font-mono text-[10.5px] uppercase tracking-[0.14em] text-muted-foreground/70">
              <div className="col-span-12 md:col-span-3">Attribute</div>
              <div className="hidden md:block col-span-4">Bare LLM scripts</div>
              <div className="hidden md:block col-span-5 text-primary">
                Riichi dispatch
              </div>
            </div>
            <ul>
              {comparison.rows.map((row, i) => (
                <li
                  key={row.label}
                  className={cn(
                    "grid grid-cols-12 gap-x-4 gap-y-1 px-5 md:px-7 py-4 md:py-5",
                    i !== comparison.rows.length - 1 && "border-b border-border",
                  )}
                >
                  <div className="col-span-12 md:col-span-3 font-mono text-[10.5px] uppercase tracking-[0.14em] pt-1 text-muted-foreground/70">
                    {row.label}
                  </div>
                  <div className="col-span-12 md:col-span-4 text-[13px] text-muted-foreground/80">
                    <span className="md:hidden font-mono text-[9.5px] mr-2 uppercase tracking-[0.14em] text-muted-foreground/50">
                      Scripts
                    </span>
                    {row.left}
                  </div>
                  <div className="col-span-12 md:col-span-5 text-[13.5px] tracking-[-0.005em] text-foreground">
                    <span className="md:hidden font-mono text-[9.5px] mr-2 uppercase tracking-[0.14em] text-primary">
                      Riichi
                    </span>
                    {row.right}
                  </div>
                </li>
              ))}
            </ul>
          </div>
          <p className="mt-3 font-mono text-[11px] text-muted-foreground/60">
            Compared against running LLM scripts against your repo. Updated as
            the dispatch protocol evolves.
          </p>
        </Reveal>
      </div>
    </section>
  );
}
