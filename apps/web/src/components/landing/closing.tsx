import { Link } from "@tanstack/react-router";
import { cn } from "@/lib/utils";
import SusukiMoonSvg from "@/components/susuki_moon";
import { buildHash, buildVersion } from "./_/data";
import { Reveal, pressable } from "./_/reveal";

export function Closing() {
  return (
    <>
      <section
        id="start"
        className="relative pt-32 md:pt-48 pb-28 md:pb-40 overflow-hidden bg-foreground text-background"
      >
        <div
          aria-hidden
          className="absolute inset-x-0 top-0 h-px bg-primary"
        />
        <div className="mx-auto max-w-[1440px] px-6 md:px-10">
          <Reveal>
            <div className="grid grid-cols-12 gap-6 mb-14">
            <div className="col-span-12 md:col-span-3 font-mono text-[10.5px] uppercase tracking-[0.16em] text-background/55">
              Begin
            </div>
              <div className="col-span-12 md:col-span-9">
                <p className="text-[13.5px] max-w-[60ch] text-background/60">
                  A short trial is enough. You will know within a cycle whether
                  Riichi fits the shape of your team. If it doesn't, you leave
                  with the same data you came in with.
                </p>
              </div>
            </div>
          </Reveal>

          <Reveal as="div" delay={80} y={20} blur={10}>
            <h2 className="text-[clamp(56px,9.5vw,148px)] leading-[0.94] tracking-[-0.035em] font-medium">
              <span className="text-background">Send the work.</span>
              <br />
              <span className="block pl-[6vw] md:pl-[8vw] text-background/60">
                Keep the why.
              </span>
            </h2>
          </Reveal>

          <Reveal as="div" delay={140} className="mt-14 md:mt-20 grid grid-cols-12 gap-6 items-end">
            <div className="col-span-12 md:col-span-7 flex flex-wrap items-center gap-3">
              <Link
                to="/login"
                className={cn(
                  pressable,
                  "inline-flex items-center gap-2 rounded-full bg-primary px-6 py-3.5 text-[15px] text-background transition-colors hover:bg-primary/80",
                )}
              >
                Start a workspace
                <span aria-hidden>→</span>
              </Link>
              <Link
                to="/login"
                className={cn(
                  pressable,
                  "inline-flex items-center gap-2 rounded-full border border-background/20 px-6 py-3.5 text-[15px] text-background transition-colors hover:bg-background/5",
                )}
              >
                Plan a migration
              </Link>
            </div>
            <div className="col-span-12 md:col-span-5 md:text-right font-mono text-[11.5px] text-background/40">
              No credit card · 2-minute setup · Linear import included · Cancel
              anytime, keep your data
            </div>
          </Reveal>
        </div>
      </section>

      <Footer />
    </>
  );
}

function Footer() {
  return (
    <footer className="bg-muted text-foreground">
      <div className="mx-auto max-w-[1440px] px-6 md:px-10">
        <div className="grid grid-cols-12 gap-6 border-b border-border py-14">
          <div className="col-span-12 md:col-span-4">
            <Link to="/" className="flex items-center gap-2.5" aria-label="Riichi home">
              <SusukiMoonSvg className="size-7" />
              <span className="text-[17px] font-medium tracking-[-0.02em]">
                Riichi
              </span>
            </Link>
            <p className="mt-5 text-[13.5px] leading-[1.55] max-w-[36ch] text-muted-foreground">
              Agent dispatch for software teams that already trust their queue
              and need to trust their agents.
            </p>
            <div className="mt-7 font-mono text-[10.5px] uppercase tracking-[0.16em] text-muted-foreground/70 space-y-1.5">
              <div className="flex items-center gap-2">
                <span className="size-1.5 rounded-full bg-emerald-400" />
                Operational
              </div>
              <div className="tabular-nums">
                {buildVersion} · #{buildHash}
              </div>
            </div>
          </div>

          <FooterCol
            title="Product"
            links={[
              { label: "Dispatch", href: "#dispatch" },
              { label: "Principles", href: "#principles" },
              { label: "Roster", href: "#roster" },
              { label: "Decisions", href: "#decisions" },
              { label: "Pricing", href: "#pricing" },
              { label: "FAQ", href: "#faq" },
            ]}
          />
          <FooterCol
            title="Resources"
            links={[
              { label: "Docs", href: "#" },
              { label: "API reference", href: "#" },
              { label: "CLI", href: "#" },
            ]}
          />
          <FooterCol
            title="Company"
            links={[
              { label: "Contact", href: "mailto:hello@riichi.app" },
              { label: "Security", href: "#" },
              { label: "Status", href: "#" },
            ]}
          />
        </div>

        <div className="py-6 flex flex-col md:flex-row items-start md:items-center justify-between gap-4 font-mono text-[11px] text-muted-foreground/70">
          <div>© 2026 Riichi Systems, Inc.</div>
          <div className="flex items-center gap-5">
            <a href="#" className="transition-colors hover:text-foreground">Terms</a>
            <a href="#" className="transition-colors hover:text-foreground">Privacy</a>
            <a href="#" className="transition-colors hover:text-foreground">DPA</a>
          </div>
        </div>
      </div>
    </footer>
  );
}

function FooterCol({
  title,
  links,
}: {
  title: string;
  links: { label: string; href: string }[];
}) {
  return (
    <div className="col-span-6 md:col-span-2 lg:col-span-2">
      <div className="font-mono text-[10.5px] uppercase tracking-[0.16em] mb-4 text-muted-foreground/70">
        {title}
      </div>
      <ul className="space-y-2 text-[13.5px] text-muted-foreground">
        {links.map((l) => (
          <li key={l.label}>
            <a
              className="transition-colors hover:text-primary"
              href={l.href}
            >
              {l.label}
            </a>
          </li>
        ))}
      </ul>
    </div>
  );
}
