import { Link } from "@tanstack/react-router";
import { cn } from "@/lib/utils";
import { Screenshot } from "./_/screenshot";
import { Reveal, pressable } from "./_/reveal";

export function Hero() {
  return (
    <section id="top" className="relative pt-14 md:pt-32 pb-20 md:pb-28">
      <div className="mx-auto max-w-[1440px] px-6 md:px-10">
        <Reveal as="div" delay={80} y={20} blur={10}>
          <h1 className="mt-8 md:mt-10 text-[clamp(48px,8vw,150px)] leading-[0.95] tracking-[-0.035em] font-medium max-w-[14ch]">
            <span className="text-foreground">Make </span>
            <span className="text-muted-foreground">software </span>
            <br />
            <span className="text-foreground">fun again.</span>
          </h1>
        </Reveal>

        <div className="mt-8 md:mt-10 flex flex-col lg:flex-row lg:items-end lg:justify-between gap-6">
          <Reveal as="div" delay={140} y={12} className="max-w-[50ch]">
            <p className="text-[18px] md:text-[21px] leading-[1.45] tracking-[-0.01em] text-foreground">
              Move faster with AI while staying firmly in control. Riichi gives agents room to act
              without turning your workflow into a black box.
            </p>

            <div className="mt-8 flex flex-wrap items-center gap-3">
              <Link
                to="/login"
                className={cn(
                  pressable,
                  "inline-flex items-center gap-2 rounded-full bg-foreground px-5 py-3 text-[14px] text-background transition-colors hover:bg-foreground/80",
                )}
              >
                Start a workspace
                <span aria-hidden>→</span>
              </Link>
              <a
                href="#dispatch"
                className={cn(
                  pressable,
                  "inline-flex items-center gap-2 rounded-full border border-border px-5 py-3 text-[14px] transition-colors hover:bg-muted",
                )}
              >
                See how it works
                <span aria-hidden>↓</span>
              </a>
            </div>
          </Reveal>

          {/*<Reveal as="div" delay={180} y={12} className="hidden lg:block">
            <a
              href="#pricing"
              className="inline-flex items-center gap-2 rounded-full border border-border bg-muted/40 px-4 py-2 text-[13px] text-muted-foreground transition-colors hover:bg-muted"
            >
              <span className="rounded bg-primary/15 px-1.5 py-0.5 font-mono text-[10px] uppercase tracking-wider text-primary">
                Plan
              </span>
              <span>Single workspace · $16</span>
              <span aria-hidden>→</span>
            </a>
          </Reveal>*/}
        </div>

        <Reveal as="div" delay={260} className="mt-16 md:mt-24" y={24} blur={8}>
          <Screenshot />
        </Reveal>
      </div>
    </section>
  );
}
