import { useId } from "react";
import { Reveal, RevealItem, useReducedMotion } from "./_/reveal";

const pillars = [
  {
    id: "decisions",
    eyebrow: "Fig 01",
    title: "Structured around decisions",
    body: "Every issue carries context, constraints, and a clear owner. Decisions become searchable history instead of lost Slack threads and ad-hoc DMs.",
    visual: DecisionTree,
  },
  {
    id: "fast",
    eyebrow: "Fig 02",
    title: "Fast by design",
    body: "Leases, fencing tokens, and automatic recovery mean agents spend less time waiting and teams spend less time debugging runaway runs.",
    visual: SpeedRing,
  },
  {
    id: "agents",
    eyebrow: "Fig 03",
    title: "Built with agents in mind",
    body: "Agents are first-class assignees with bounded capabilities. They get exactly what they need to move an issue forward — nothing more.",
    visual: AgentPair,
  },
] as const;

export function Values() {
  return (
    <section id="values" className="relative py-20 md:py-28 border-t border-border">
      <div className="mx-auto max-w-[1440px] px-6 md:px-10">
        <Reveal>
          <h2 className="text-[clamp(36px,5.5vw,68px)] leading-[1.05] tracking-[-0.025em] font-medium max-w-[32ch]">
            <span className="text-foreground">Software engineering should feel like building.</span>{" "}
            <span className="text-muted-foreground">
              Purpose-built for teams that want agents to handle the process without creating more unneeded process.
            </span>
          </h2>
        </Reveal>

        <div className="mt-16 md:mt-24 grid grid-cols-12 gap-8 md:gap-6">
          {pillars.map((pillar, i) => (
            <RevealItem
              key={pillar.id}
              delay={i * 100}
              className="col-span-12 md:col-span-6 lg:col-span-4"
            >
              <article className="flex h-full flex-col">
                <div className="relative aspect-[4/3] overflow-hidden rounded-[16px] border border-border bg-muted/20">
                  <pillar.visual />
                </div>
                <div className="mt-5 font-mono text-[10px] uppercase tracking-[0.16em] text-muted-foreground/60">
                  {pillar.eyebrow}
                </div>
                <h3 className="mt-2 text-[20px] md:text-[24px] tracking-[-0.015em] font-medium">
                  {pillar.title}
                </h3>
                <p className="mt-3 text-[14px] leading-[1.6] text-muted-foreground">
                  {pillar.body}
                </p>
              </article>
            </RevealItem>
          ))}
        </div>
      </div>
    </section>
  );
}

/* ---------- visuals ---------- */

function DecisionTree() {
  const reduced = useReducedMotion();
  const gid = useId();
  return (
    <svg
      viewBox="0 0 200 150"
      className="absolute inset-0 h-full w-full p-6"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
    >
      <defs>
        <linearGradient id={`${gid}-grad`} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0%" stopColor="var(--foreground)" stopOpacity="0.5" />
          <stop offset="100%" stopColor="var(--primary)" stopOpacity="0.8" />
        </linearGradient>
      </defs>

      {/* trunk */}
      <path
        d="M100 24 L100 70 L52 110"
        stroke={`url(#${gid}-grad)`}
        strokeWidth="1.5"
        strokeLinecap="round"
        className="opacity-40"
      />
      <path
        d="M100 70 L148 110"
        stroke={`url(#${gid}-grad)`}
        strokeWidth="1.5"
        strokeLinecap="round"
        className="opacity-40"
      />

      {/* nodes */}
      {[100, 52, 148].map((cx, i) => {
        const cy = i === 0 ? 24 : 110;
        return (
          <g key={i}>
            <circle cx={cx} cy={cy} r="10" fill="var(--background)" stroke="var(--foreground)" strokeWidth="1.5" strokeOpacity="0.6" />
            {!reduced && (
              <circle cx={cx} cy={cy} r="10" fill="none" stroke="var(--primary)" strokeWidth="1.5" strokeOpacity="0">
                <animate attributeName="r" values="10;18;10" dur="3s" repeatCount="indefinite" begin={`${i * 0.6}s`} />
                <animate attributeName="stroke-opacity" values="0.6;0;0.6" dur="3s" repeatCount="indefinite" begin={`${i * 0.6}s`} />
              </circle>
            )}
          </g>
        );
      })}

      {/* decision labels */}
      <text x="100" y="145" textAnchor="middle" fill="var(--muted-foreground)" fontSize="8" fontFamily="var(--font-mono)" letterSpacing="0.08em">
        DECIDE
      </text>
    </svg>
  );
}

function SpeedRing() {
  const reduced = useReducedMotion();
  return (
    <svg
      viewBox="0 0 200 150"
      className="absolute inset-0 h-full w-full p-6"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
    >
      <circle
        cx="100"
        cy="75"
        r="48"
        stroke="var(--foreground)"
        strokeWidth="1"
        strokeOpacity="0.15"
        strokeDasharray="4 4"
      />
      {!reduced && (
        <circle
          cx="100"
          cy="75"
          r="48"
          stroke="var(--primary)"
          strokeWidth="1.5"
          strokeDasharray="4 4"
          strokeLinecap="round"
        >
          <animateTransform attributeName="transform" type="rotate" from="0 100 75" to="360 100 75" dur="12s" repeatCount="indefinite" />
        </circle>
      )}
      <circle
        cx="100"
        cy="75"
        r="28"
        stroke="var(--foreground)"
        strokeWidth="1.5"
        strokeOpacity="0.4"
      />
      <path
        d="M100 55 L100 75 L116 83"
        stroke="var(--primary)"
        strokeWidth="2"
        strokeLinecap="round"
        strokeLinejoin="round"
      />

      {/* speed lines */}
      {[40, 160].map((x, i) => (
        <g key={i} opacity="0.25">
          <line x1={x} y1="45" x2={x} y2="105" stroke="var(--foreground)" strokeWidth="1" />
          <line x1={x + (i === 0 ? -8 : 8)} y1="55" x2={x + (i === 0 ? -8 : 8)} y2="95" stroke="var(--foreground)" strokeWidth="1" />
        </g>
      ))}

      <text x="100" y="138" textAnchor="middle" fill="var(--muted-foreground)" fontSize="8" fontFamily="var(--font-mono)" letterSpacing="0.08em">
        DISPATCH
      </text>
    </svg>
  );
}

function AgentPair() {
  const reduced = useReducedMotion();
  return (
    <svg
      viewBox="0 0 200 150"
      className="absolute inset-0 h-full w-full p-6"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
    >
      {/* connection line */}
      <line x1="58" y1="75" x2="142" y2="75" stroke="var(--foreground)" strokeWidth="1" strokeOpacity="0.2" />
      {!reduced && (
        <circle cx="58" cy="75" r="3" fill="var(--primary)">
          <animate attributeName="cx" values="58;142;58" dur="2.5s" repeatCount="indefinite" />
        </circle>
      )}

      {/* human avatar */}
      <g transform="translate(58, 75)">
        <circle r="18" fill="var(--background)" stroke="var(--foreground)" strokeWidth="1.5" strokeOpacity="0.5" />
        <circle r="7" fill="var(--foreground)" fillOpacity="0.8" />
        <path d="M-18 18 Q0 28 18 18" stroke="var(--foreground)" strokeWidth="1.5" strokeOpacity="0.5" fill="none" />
      </g>

      {/* agent avatar */}
      <g transform="translate(142, 75)">
        <circle r="18" fill="var(--background)" stroke="var(--primary)" strokeWidth="1.5" />
        <path d="M-7 -2 L0 -8 L7 -2 L4 7 L-4 7 Z" fill="var(--primary)" />
      </g>

      {/* labels */}
      <text x="58" y="115" textAnchor="middle" fill="var(--muted-foreground)" fontSize="8" fontFamily="var(--font-mono)" letterSpacing="0.08em">
        HUMAN
      </text>
      <text x="142" y="115" textAnchor="middle" fill="var(--muted-foreground)" fontSize="8" fontFamily="var(--font-mono)" letterSpacing="0.08em">
        AGENT
      </text>
    </svg>
  );
}
