import {
  createContext,
  useContext,
  useEffect,
  useRef,
  useState,
  type ElementType,
  type ReactNode,
} from "react";
import { cn } from "@/lib/utils";

/**
 * Reveal animations for the landing page.
 *
 * Built on IntersectionObserver + CSS transitions (no framer-motion):
 *   - off-main-thread friendly (CSS transitions don't block under load)
 *   - once-only via observer disconnect
 *   - reduced motion: jump to visible state immediately, keep color/opacity
 *     transitions for state changes, drop transform/blur
 */

const EASE_OUT = "cubic-bezier(0.22, 1, 0.36, 1)";

export function useReducedMotion(): boolean {
  const [reduced, setReduced] = useState(false);
  useEffect(() => {
    const mql = window.matchMedia("(prefers-reduced-motion: reduce)");
    const update = () => setReduced(mql.matches);
    update();
    mql.addEventListener("change", update);
    return () => mql.removeEventListener("change", update);
  }, []);
  return reduced;
}

interface RevealCtx {
  visible: boolean;
  reduced: boolean;
}
const Ctx = createContext<RevealCtx>({ visible: false, reduced: false });

interface RevealProps {
  children: ReactNode;
  as?: ElementType;
  delay?: number;
  className?: string;
  /** Translate Y in px before reveal. */
  y?: number;
  /** Blur in px before reveal. */
  blur?: number;
}

export function Reveal({
  children,
  as,
  delay = 0,
  className,
  y = 16,
  blur = 6,
}: RevealProps) {
  const Tag = (as ?? "div") as ElementType;
  const ref = useRef<HTMLElement | null>(null);
  const [visible, setVisible] = useState(false);
  const reduced = useReducedMotion();

  useEffect(() => {
    const el = ref.current;
    if (!el || reduced) {
      setVisible(true);
      return;
    }
    const io = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) {
            setVisible(true);
            io.disconnect();
            break;
          }
        }
      },
      { rootMargin: "-80px 0px", threshold: 0.05 },
    );
    io.observe(el);
    return () => io.disconnect();
  }, [reduced]);

  const style = reduced
    ? undefined
    : {
        transitionProperty: "opacity, transform, filter",
        transitionDuration: "720ms",
        transitionTimingFunction: EASE_OUT,
        transitionDelay: `${delay}ms`,
        opacity: visible ? 1 : 0,
        transform: visible ? "none" : `translateY(${y}px)`,
        filter: visible ? "blur(0px)" : `blur(${blur}px)`,
        willChange: "opacity, transform, filter",
      };

  return (
    <Ctx.Provider value={{ visible, reduced }}>
      <Tag ref={ref as never} className={className} style={style}>
        {children}
      </Tag>
    </Ctx.Provider>
  );
}

/**
 * Paired with <Reveal>: child reveals that should fire in sequence after
 * their parent. Use for staggered lists where each item is its own observer.
 */
export function RevealItem({
  children,
  className,
  delay = 0,
  y = 12,
}: {
  children: ReactNode;
  className?: string;
  delay?: number;
  y?: number;
}) {
  const ref = useRef<HTMLElement | null>(null);
  const [visible, setVisible] = useState(false);
  const reduced = useReducedMotion();

  useEffect(() => {
    const el = ref.current;
    if (!el || reduced) {
      setVisible(true);
      return;
    }
    const io = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) {
            setVisible(true);
            io.disconnect();
            break;
          }
        }
      },
      { rootMargin: "-40px 0px", threshold: 0.1 },
    );
    io.observe(el);
    return () => io.disconnect();
  }, [reduced]);

  const style = reduced
    ? undefined
    : {
        transitionProperty: "opacity, transform, filter",
        transitionDuration: "600ms",
        transitionTimingFunction: EASE_OUT,
        transitionDelay: `${delay}ms`,
        opacity: visible ? 1 : 0,
        transform: visible ? "none" : `translateY(${y}px)`,
        filter: visible ? "blur(0px)" : "blur(4px)",
        willChange: "opacity, transform, filter",
      };

  return (
    <div ref={ref as never} className={className} style={style}>
      {children}
    </div>
  );
}

export function useRevealState() {
  return useContext(Ctx);
}

/** Optional media query hook for hover capabilities. */
export function useHoverCapable(): boolean {
  const [capable, setCapable] = useState(true);
  useEffect(() => {
    const mql = window.matchMedia("(hover: hover) and (pointer: fine)");
    const update = () => setCapable(mql.matches);
    update();
    mql.addEventListener("change", update);
    return () => mql.removeEventListener("change", update);
  }, []);
  return capable;
}

/** Convenience: classes for pressable things. */
export const pressable = "transition-transform duration-150 active:scale-[0.97]";
