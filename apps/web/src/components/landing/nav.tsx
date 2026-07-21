import { useEffect, useState } from "react";
import { Link } from "@tanstack/react-router";
import { cn } from "@/lib/utils";
import SusukiMoonSvg from "@/components/susuki_moon";
import { buildHash, buildVersion } from "./_/data";
import { pressable } from "./_/reveal";

export function LandingNav() {
  const [scrolled, setScrolled] = useState(false);
  const [open, setOpen] = useState(false);

  useEffect(() => {
    const onScroll = () => setScrolled(window.scrollY > 8);
    onScroll();
    window.addEventListener("scroll", onScroll, { passive: true });
    return () => window.removeEventListener("scroll", onScroll);
  }, []);

  return (
    <header
      className={cn(
        "sticky top-0 z-50 transition-colors duration-300",
        scrolled ? "bg-background/85 backdrop-blur-md" : "bg-transparent",
      )}
    >
      <div className="border-b border-border">
        <div className="mx-auto max-w-[1440px] px-6 md:px-10">
          <nav className="flex h-16 items-center justify-between gap-6">
            <Link
              to="/"
              className="flex items-center gap-2.5"
              aria-label="Riichi home"
            >
              <SusukiMoonSvg className="size-7" />
              <span className="text-[17px] font-medium tracking-[-0.02em]">
                Riichi
              </span>
            </Link>

            <div className="flex items-center gap-4">
              <Link
                to="/login"
                className="hidden sm:inline-flex rounded-full px-3.5 py-2 text-[13.5px] transition-colors hover:bg-muted"
              >
                Sign in
              </Link>
              <Link
                to="/login"
                className={cn(
                  pressable,
                  "inline-flex items-center gap-2 rounded-full bg-foreground px-4 py-2 text-[13.5px] text-background transition-colors hover:bg-foreground/80",
                )}
              >
                Start a workspace
                <span aria-hidden>→</span>
              </Link>
              <button
                type="button"
                className="lg:hidden rounded-full border border-border p-2"
                aria-label="Toggle navigation"
                aria-expanded={open}
                onClick={() => setOpen((v) => !v)}
              >
                <span className="block w-4">
                  <span
                    className={cn(
                      "block h-px bg-foreground transition-transform",
                      open && "translate-y-[3px] rotate-45",
                    )}
                  />
                  <span
                    className={cn(
                      "block h-px bg-foreground transition-transform mt-[5px]",
                      open && "-translate-y-[3px] -rotate-45",
                    )}
                  />
                </span>
              </button>
            </div>
          </nav>

        </div>
      </div>
    </header>
  );
}
