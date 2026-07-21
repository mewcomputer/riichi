import { LandingNav } from "@/components/landing/nav";
import { Hero } from "@/components/landing/hero";
import { Values } from "@/components/landing/values";
import { Principles } from "@/components/landing/principles";
import { Roster } from "@/components/landing/roster";
import { DispatchLifecycle } from "@/components/landing/dispatch-lifecycle";
import { DecisionsTeaser } from "@/components/landing/decisions";
import { Pricing } from "@/components/landing/pricing";
import { FAQ } from "@/components/landing/faq";
import { Closing } from "@/components/landing/closing";

export function LandingPage() {
  return (
    <div className="min-h-screen bg-background text-foreground">
      <LandingNav />
      <main>
        <Hero />
        <Values />
        <Principles />
        <Roster />
        <DispatchLifecycle />
        <DecisionsTeaser />
        <Pricing />
        <FAQ />
        <Closing />
      </main>
    </div>
  );
}
