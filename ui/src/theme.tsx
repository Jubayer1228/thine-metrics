import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";

export type ThemeMode = "light" | "dark" | "system";
export type ResolvedTheme = "light" | "dark";

type ThemeCtx = {
  mode: ThemeMode;
  resolved: ResolvedTheme;
  setMode: (mode: ThemeMode) => void;
  cycle: () => void;
};

const ThemeContext = createContext<ThemeCtx | null>(null);
const STORAGE_KEY = "thine-theme-mode";

function systemPrefersDark(): boolean {
  return typeof window !== "undefined" && window.matchMedia("(prefers-color-scheme: dark)").matches;
}

function resolve(mode: ThemeMode): ResolvedTheme {
  if (mode === "system") return systemPrefersDark() ? "dark" : "light";
  return mode;
}

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [mode, setModeState] = useState<ThemeMode>(() => {
    try {
      const saved = localStorage.getItem(STORAGE_KEY) as ThemeMode | null;
      if (saved === "light" || saved === "dark" || saved === "system") return saved;
    } catch {
      /* ignore */
    }
    return "system";
  });
  const [resolved, setResolved] = useState<ResolvedTheme>(() => resolve(mode));

  const setMode = useCallback((next: ThemeMode) => {
    setModeState(next);
    try {
      localStorage.setItem(STORAGE_KEY, next);
    } catch {
      /* ignore */
    }
  }, []);

  const cycle = useCallback(() => {
    setMode(mode === "light" ? "dark" : mode === "dark" ? "system" : "light");
  }, [mode, setMode]);

  useEffect(() => {
    const apply = () => {
      const r = resolve(mode);
      setResolved(r);
      document.documentElement.setAttribute("data-theme", r);
      document.documentElement.style.colorScheme = r;
    };
    apply();
    if (mode !== "system") return;
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = () => apply();
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, [mode]);

  const value = useMemo(() => ({ mode, resolved, setMode, cycle }), [mode, resolved, setMode, cycle]);

  return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>;
}

export function useTheme(): ThemeCtx {
  const ctx = useContext(ThemeContext);
  if (!ctx) throw new Error("useTheme must be used within ThemeProvider");
  return ctx;
}

export function ThemeToggle() {
  const { mode, resolved, cycle } = useTheme();
  const label = mode === "system" ? `System (${resolved})` : mode === "dark" ? "Dark" : "Light";
  const icon = resolved === "dark" ? "☾" : "☀";
  return (
    <button
      type="button"
      className="ghost theme-toggle"
      onClick={cycle}
      title={`Theme: ${label} — click to cycle Light / Dark / System`}
      aria-label={`Theme ${label}`}
    >
      <span aria-hidden>{icon}</span>
      <span className="theme-toggle-label">{label}</span>
    </button>
  );
}

/** Recharts tooltip / axis styles that follow CSS theme tokens */
export const chartTip = {
  background: "var(--surf-elevated)",
  border: "1px solid var(--surf-border)",
  borderRadius: 6,
  color: "var(--surf-text)",
  fontSize: 12,
} as const;

export const chartTick = { fill: "var(--surf-muted)", fontSize: 11 } as const;
export const chartGrid = "var(--surf-border)";

