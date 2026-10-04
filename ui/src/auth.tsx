import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";

export type Session = {
  token: string;
  user_email: string;
  roles: string[];
  org_id: string;
  org_name: string;
  ingest_api_key: string;
};

export type OnboardingGuide = {
  org_id: string;
  org_name: string;
  ingest_api_key: string;
  site_url: string;
  onboarding: { step: string; completed: string[] };
  steps: { id: string; title: string; detail: string; done: boolean }[];
};

const STORAGE_KEY = "thine_session_v1";

type AuthContextValue = {
  session: Session | null;
  setSession: (s: Session | null) => void;
  authHeaders: () => Record<string, string>;
  signOut: () => void;
};

const AuthContext = createContext<AuthContextValue | null>(null);

export function AuthProvider({ children }: { children: ReactNode }) {
  const [session, setSessionState] = useState<Session | null>(() => {
    try {
      const raw = localStorage.getItem(STORAGE_KEY);
      return raw ? (JSON.parse(raw) as Session) : null;
    } catch {
      return null;
    }
  });

  const setSession = useCallback((s: Session | null) => {
    setSessionState(s);
    if (s) localStorage.setItem(STORAGE_KEY, JSON.stringify(s));
    else localStorage.removeItem(STORAGE_KEY);
  }, []);

  const signOut = useCallback(() => setSession(null), [setSession]);

  const authHeaders = useCallback((): Record<string, string> => {
    if (!session?.token) return {};
    return { Authorization: `Bearer ${session.token}` };
  }, [session]);

  useEffect(() => {
    const hash = window.location.hash;
    const qs = hash.includes("?") ? hash.slice(hash.indexOf("?") + 1) : "";
    const params = new URLSearchParams(qs);
    const token = params.get("token");
    if (!token) return;
    fetch("/api/v1/auth/me", { headers: { Authorization: `Bearer ${token}` } })
      .then((r) => (r.ok ? r.json() : null))
      .then((tok) => {
        if (tok?.token) {
          setSession(tok as Session);
          window.location.hash = "#get-started";
        }
      })
      .catch(() => undefined);
  }, [setSession]);

  const value = useMemo(
    () => ({ session, setSession, authHeaders, signOut }),
    [session, setSession, authHeaders, signOut],
  );

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}

export function useAuth() {
  const ctx = useContext(AuthContext);
  if (!ctx) throw new Error("useAuth requires AuthProvider");
  return ctx;
}
