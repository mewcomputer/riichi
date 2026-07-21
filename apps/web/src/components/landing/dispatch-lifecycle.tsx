import { useEffect, useState } from "react";
import { cn } from "@/lib/utils";
import { dispatchLifecycle } from "./_/data";
import { Reveal, RevealItem, useReducedMotion, pressable } from "./_/reveal";

const STEP_MS = 5200;

export function DispatchLifecycle() {
  const [step, setStep] = useState(0);
  const reduced = useReducedMotion();

  useEffect(() => {
    if (reduced) return;
    const id = setInterval(() => {
      setStep((s) => (s + 1) % dispatchLifecycle.steps.length);
    }, STEP_MS);
    return () => clearInterval(id);
  }, [reduced]);

  const current = dispatchLifecycle.steps[step];

  return (
    <section id="dispatch" className="relative py-20 md:py-28 border-t border-border">
      <div className="mx-auto max-w-[1440px] px-6 md:px-10">
        <Reveal>
          <div className="font-mono text-[10.5px] uppercase tracking-[0.16em] text-muted-foreground/60">
            Dispatch
          </div>
          <h2 className="mt-3 text-[clamp(36px,5.5vw,68px)] leading-[1.02] tracking-[-0.025em] font-medium max-w-[18ch]">
            <span className="text-foreground">Autonomy</span>
            <br className="hidden sm:block" />
            <span className="text-muted-foreground">with guardrails.</span>
          </h2>
          <p className="mt-5 max-w-[58ch] text-[14.5px] leading-[1.55] text-muted-foreground">
            Let agents work on real issues without giving up control. Every lease
            carries a fencing token, so stale writes are rejected automatically and
            recovered agents can't overwrite work that moved on.
          </p>
        </Reveal>

        <Reveal as="div" delay={120} className="mt-14 md:mt-20">
          <div className="grid grid-cols-12 gap-0 overflow-hidden rounded-[18px] border border-border">
            {/* Step list */}
            <div className="col-span-12 lg:col-span-5 bg-muted/30">
              <ul
                role="tablist"
                aria-label="Dispatch lifecycle steps"
                className="divide-y divide-border"
              >
                {dispatchLifecycle.steps.map((s, i) => {
                  const isActive = i === step;
                  const isPast = i < step;
                  return (
                    <li key={s.n}>
                      <button
                        type="button"
                        role="tab"
                        aria-selected={isActive}
                        onClick={() => setStep(i)}
                        className={cn(
                          pressable,
                          "relative w-full text-left px-5 md:px-7 py-6 md:py-7 transition-colors",
                          isActive ? "bg-background" : "hover:bg-background/60",
                        )}
                      >
                        <div className="flex items-baseline gap-3">
                          <span
                            className={cn(
                              "font-mono text-[10.5px] uppercase tracking-[0.16em]",
                              isActive ? "text-foreground" : "text-muted-foreground/70",
                            )}
                          >
                            {s.label}
                          </span>
                          {isPast && !isActive && (
                            <span className="ml-auto font-mono text-[10.5px] text-emerald-300/80">
                              ✓
                            </span>
                          )}
                        </div>
                        <h3 className="mt-2 text-[18px] md:text-[20px] tracking-[-0.01em] font-medium">
                          {s.title}
                        </h3>
                        <p
                          className={cn(
                            "mt-2 text-[13px] leading-[1.55] max-w-[42ch]",
                            isActive ? "text-muted-foreground" : "text-muted-foreground/70",
                          )}
                        >
                          {s.body}
                        </p>
                        <span
                          aria-hidden
                          className={cn(
                            "absolute bottom-0 left-0 h-px bg-primary transition-[width] duration-500 ease-out",
                            isActive ? "w-full" : "w-0",
                          )}
                        />
                      </button>
                    </li>
                  );
                })}
              </ul>
            </div>

            {/* Terminal */}
            <div className="col-span-12 lg:col-span-7 min-h-[440px] lg:min-h-[540px] relative bg-background">
              <Terminal step={step} />
            </div>
          </div>
        </Reveal>

        <RevealItem className="mt-4 flex items-center gap-3 font-mono text-[11px] text-muted-foreground/60">
          <span>Issue · ENG-224</span>
          <span aria-hidden>·</span>
          <span className="tabular-nums">{current.detail}</span>
        </RevealItem>
      </div>
    </section>
  );
}

/* ---------- terminal ---------- */

const sessions: Record<number, TerminalSession> = {
  0: {
    command: "riichi-agent ready --limit 5",
    output: [
      "ISSUE              STATUS   STATE   AGE",
      "ENG-218            todo     ready   2h 14m",
      "ENG-224            todo     ready   6h 02m  ←",
      "ENG-242            todo     ready   3h 48m",
      "",
      "2 issues ready for dispatch on Platform.",
    ],
  },
  1: {
    command: "riichi-agent claim eng-224 --ttl-seconds 7200",
    output: [
      "lease:   0x4a2f",
      "agent:   platform-builder",
      "ttl:     120m",
      "caps:    discover, complete, doc.apply_edit, release",
      "",
      "Token minted. Write fence active.",
    ],
  },
  2: {
    command: "riichi-agent report 0x4a2f --operations '[...]'",
    output: [
      "reports: 3 submitted",
      "verified: 3",
      "rejected: 0",
      "",
      "All writes carry token 0x4a2f.",
    ],
  },
  3: {
    command: "riichi-agent context eng-224",
    output: [
      "status:  done",
      "lease:   released",
      "token:   0x4a2f retired",
      "next:    0x4a30",
      "",
      "Issue closed. Fence reset.",
    ],
  },
};

interface TerminalSession {
  command: string;
  output: string[];
}

function Terminal({ step }: { step: number }) {
  const reduced = useReducedMotion();
  const session = sessions[step];
  const [typed, setTyped] = useState(reduced ? session.command : "");
  const [showOutput, setShowOutput] = useState(reduced);

  useEffect(() => {
    if (reduced) {
      setTyped(session.command);
      setShowOutput(true);
      return;
    }
    setTyped("");
    setShowOutput(false);
    let i = 0;
    const commandId = setInterval(() => {
      i += 1;
      setTyped(session.command.slice(0, i));
      if (i >= session.command.length) {
        clearInterval(commandId);
        setTimeout(() => setShowOutput(true), 250);
      }
    }, 35);
    return () => clearInterval(commandId);
  }, [step, session.command, reduced]);

  return (
    <div className="absolute inset-0 flex flex-col p-6 md:p-8">
      <div className="flex items-center gap-3">
        <div className="flex items-center gap-1.5">
          <span className="size-2.5 rounded-full bg-muted-foreground/30" />
          <span className="size-2.5 rounded-full bg-muted-foreground/25" />
          <span className="size-2.5 rounded-full bg-muted-foreground/20" />
        </div>
        <div className="ml-2 rounded-full border border-border bg-muted/50 px-2.5 py-1 font-mono text-[10.5px] text-muted-foreground/70">
          platform-builder@northwind
        </div>
      </div>

      <div className="mt-6 flex-1 rounded-[14px] border border-border bg-muted/30 p-4 md:p-5 font-mono text-[13px] leading-relaxed">
        <div className="flex items-center gap-2 text-foreground">
          <span className="text-muted-foreground/50">$</span>
          <span>{typed}</span>
          {!showOutput && (
            <span className="caret ml-px inline-block h-[15px] w-[7px] bg-primary align-middle" />
          )}
        </div>

        <div
          className={cn(
            "mt-4 space-y-1 text-muted-foreground/80 transition-opacity duration-300",
            showOutput ? "opacity-100" : "opacity-0",
          )}
        >
          {session.output.map((line, i) => (
            <div key={`${step}-${i}`} className={cn(line === "" && "h-3")}>
              {line}
            </div>
          ))}
        </div>
      </div>

      <div className="mt-4 flex items-center gap-2 font-mono text-[11px] text-muted-foreground/60">
        <span className="size-1.5 rounded-full bg-primary" />
        <span className="tabular-nums">Token {step === 0 ? "—" : step === 3 ? "retired" : "0x4a2f"}</span>
        <span className="text-muted-foreground/40">·</span>
        <span>{step === 0 ? "Ready" : step === 3 ? "Released" : "Active"}</span>
      </div>
    </div>
  );
}
