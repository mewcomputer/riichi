import { ShieldCheck } from "lucide-react";
import { Link } from "@tanstack/react-router";
import { cn } from "@/lib/utils";
import SusukiMoonSvg from "@/components/susuki_moon";
import { Reveal, pressable } from "./_/reveal";

const features = [
  "Unlimited issues, agents, and projects",
  "Capability-bounded agent roles",
  "Fenced claims, leases, and reports",
  "Holds, approvals, and revocations",
  "Recovery and quarantine on failure",
  "Activity-based project reporting",
  "Linear, Jira, and Shortcut import",
  "First-party CLI",
];

export function Pricing() {
  return (
    <section id="pricing" className="relative py-20 md:py-28">
      <div className="mx-auto max-w-[1440px] px-6 md:px-10">
        <Reveal>
          <div className="font-mono text-[10.5px] uppercase tracking-[0.16em] text-muted-foreground/60">
            Plan
          </div>
          <h2 className="mt-3 text-[clamp(36px,5.5vw,68px)] leading-[1.02] tracking-[-0.025em] font-medium max-w-[18ch]">
            <span className="text-foreground">One plan.</span>
            <br className="hidden sm:block" />
            <span className="text-muted-foreground">The complete system.</span>
          </h2>
          <p className="mt-5 max-w-[58ch] text-[14.5px] leading-[1.55] text-muted-foreground">
            No feature maze. Start with a 30-day trial, import a real project,
            and decide with evidence.
          </p>
        </Reveal>

        <Reveal as="div" delay={120} className="mt-14 md:mt-20">
          <div className="rounded-[16px] bg-card overflow-hidden">
            <div className="grid grid-cols-12 gap-6 md:gap-10 p-7 md:p-10">
              {/* Identity */}
              <div className="col-span-12 lg:col-span-4 flex items-start gap-4">
                <SusukiMoonSvg className="size-9 shrink-0" />
                <div>
                  <div className="text-[16px] font-medium tracking-[-0.005em]">
                    Riichi Workspace
                  </div>
                  <div className="mt-1 text-[12.5px] text-muted-foreground/70">
                    For teams running agents against real work.
                  </div>
                  <div className="mt-5 flex items-baseline gap-2">
                    <span className="text-[44px] leading-none tracking-[-0.03em] font-medium tabular-nums">
                      $16
                    </span>
                    <span className="font-mono text-[11.5px] text-muted-foreground/70 leading-tight">
                      per member
                      <br />
                      per month
                    </span>
                  </div>
                </div>
              </div>

              {/* Features */}
              <ul className="col-span-12 lg:col-span-5 grid grid-cols-1 sm:grid-cols-2 gap-x-6 gap-y-2 self-center">
                {features.map((f) => (
                  <li
                    key={f}
                    className="flex items-start gap-2 text-[13px] text-muted-foreground"
                  >
                    <span
                      aria-hidden
                      className="mt-1.5 inline-block h-1 w-3 rounded-full bg-primary/70 shrink-0"
                    />
                    <span className="leading-[1.45]">{f}</span>
                  </li>
                ))}
              </ul>

              {/* CTA */}
              <div className="col-span-12 lg:col-span-3 flex flex-col justify-between gap-5 self-stretch">
                <Link
                  to="/login"
                  className={cn(
                    pressable,
                    "inline-flex items-center justify-center gap-2 rounded-full bg-foreground px-5 py-3.5 text-[14px] text-background transition-colors hover:bg-foreground/80",
                  )}
                >
                  Start 30-day trial
                  <span aria-hidden>→</span>
                </Link>
                <Link
                  to="/login"
                  className="inline-flex items-center justify-center gap-1 text-[13.5px] text-muted-foreground transition-colors hover:text-foreground"
                >
                  Talk to us
                  <span aria-hidden>→</span>
                </Link>
                <div className="text-center font-mono text-[11px] text-muted-foreground/60">
                  No card · Cancel anytime
                </div>
              </div>
            </div>
          </div>

          <p className="mt-5 flex items-center gap-2 font-mono text-[11.5px] text-muted-foreground/70">
            <ShieldCheck className="size-3.5 text-primary" aria-hidden />
            SSO / SAML, SCIM, audit logs, custom retention, and data residency
            available for larger organizations.
          </p>
        </Reveal>
      </div>
    </section>
  );
}
