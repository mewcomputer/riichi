import { ChevronDown } from "lucide-react";
import { cn } from "@/lib/utils";
import { faqs } from "./_/data";
import { Reveal } from "./_/reveal";

export function FAQ() {
  return (
    <section id="faq" className="relative py-20 md:py-28">
      <div className="mx-auto max-w-[1440px] px-6 md:px-10">
        <div className="grid grid-cols-12 gap-6 md:gap-10">
          <Reveal as="div" className="col-span-12 lg:col-span-4">
            <div className="font-mono text-[10.5px] uppercase tracking-[0.16em] text-muted-foreground/60">
              Questions
            </div>
            <h2 className="mt-3 text-[clamp(34px,5vw,52px)] leading-[1.05] tracking-[-0.025em] font-medium max-w-[14ch]">
              <span className="text-foreground">What teams</span>
              <br className="hidden sm:block" />
              <span className="text-muted-foreground">actually ask.</span>
            </h2>
            <p className="mt-5 text-[13.5px] leading-[1.55] max-w-[40ch] text-muted-foreground">
              If yours isn't here, write to{" "}
              <a
                className="underline underline-offset-2 text-foreground"
                href="mailto:hello@riichi.app"
              >
                hello@riichi.app
              </a>
              . Humans reply.
            </p>
          </Reveal>

          <Reveal as="div" delay={120} className="col-span-12 lg:col-span-8">
            <ul className="border-t border-foreground">
              {faqs.map((item, i) => (
                <li key={item.q} className="border-b border-border">
                  <details className="group">
                    <summary
                      className={cn(
                        "flex cursor-pointer list-none items-start gap-4 py-5 md:py-6 transition-colors",
                        "hover:text-primary",
                      )}
                    >
                      <span className="font-mono text-[11px] tabular-nums pt-1.5 shrink-0 text-muted-foreground/60">
                        {String(i + 1).padStart(2, "0")}
                      </span>
                      <span className="flex-1 text-[16.5px] md:text-[18px] leading-[1.35] tracking-[-0.01em] font-medium">
                        {item.q}
                      </span>
                      <span
                        className="shrink-0 mt-1 size-6 rounded-full border border-border grid place-items-center transition-transform duration-200 text-muted-foreground group-open:rotate-180 group-open:text-primary"
                        aria-hidden
                      >
                        <ChevronDown className="size-3.5" />
                      </span>
                    </summary>
                    <div className="pb-6 pl-9 pr-9 text-[14px] leading-[1.65] max-w-[64ch] text-muted-foreground">
                      {item.a}
                    </div>
                  </details>
                </li>
              ))}
            </ul>
          </Reveal>
        </div>
      </div>
    </section>
  );
}
