import { useCallback, useEffect, useState } from "react";
import { api, type OnboardingGuide } from "./api";
import { useAuth } from "./auth";

type Props = {
  onInstallAgent: () => void;
  onOpenDashboards: () => void;
};

export function GetStartedPage({ onInstallAgent, onOpenDashboards }: Props) {
  const { session, setSession, signOut } = useAuth();
  const [guide, setGuide] = useState<OnboardingGuide | null>(null);
  const [demoHint, setDemoHint] = useState<string | null>(null);
  const [googleEnabled, setGoogleEnabled] = useState(false);
  const [email, setEmail] = useState("");
  const [orgName, setOrgName] = useState("");
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [agents, setAgents] = useState(0);

  const reload = useCallback(() => {
    api
      .onboarding()
      .then((g) => {
        if (g && "demo" in g && g.demo) {
          setDemoHint(g.message);
          setGuide(null);
        } else {
          setDemoHint(null);
          setGuide(g as OnboardingGuide);
        }
      })
      .catch(() => setGuide(null));
    api
      .fleetAgents()
      .then((a) => setAgents(a.length))
      .catch(() => setAgents(0));
  }, []);

  useEffect(() => {
    api.authConfig().then((c) => setGoogleEnabled(c.google_enabled)).catch(() => undefined);
    reload();
  }, [reload, session]);

  const signup = async () => {
    setBusy(true);
    setErr(null);
    try {
      const { session: s } = await api.authSignup(email.trim(), orgName.trim() || undefined);
      setSession({
        token: s.token,
        user_email: s.user_email,
        roles: s.roles,
        org_id: s.org_id,
        org_name: s.org_name,
        ingest_api_key: s.ingest_api_key,
      });
      reload();
    } catch (e) {
      setErr(e instanceof Error ? e.message : "Signup failed");
    } finally {
      setBusy(false);
    }
  };

  const copyKey = (key: string) => {
    navigator.clipboard?.writeText(key).catch(() => undefined);
    if (session) api.onboardingAdvance("api_key").then(() => reload()).catch(() => undefined);
  };

  const ingestCurl =
    guide?.ingest_api_key &&
    `curl -s -X POST ${guide.site_url}/api/v1/ingest \\
  -H "content-type: application/json" \\
  -H "DD-API-KEY: ${guide.ingest_api_key}" \\
  -d '[{"name":"app.requests","value":1,"tags":{"service":"my-app","env":"prod"}}]'`;

  return (
    <section className="dd-panel get-started">
      <div className="gs-hero">
        <div>
          <h2>Get started with your own workspace</h2>
          <p className="muted">
            Sign in to get an isolated org — metrics, agents, notebooks, and dashboards never mix with
            other customers or the public demo.
          </p>
        </div>
        {session ? (
          <div className="gs-trial">
            <span>
              {session.org_name} · <code>{session.org_id}</code>
            </span>
            <button type="button" className="ghost" onClick={signOut}>
              Sign out
            </button>
          </div>
        ) : (
          <div className="gs-auth-card">
            {googleEnabled ? (
              <a className="gs-primary" href="/api/v1/auth/oauth/google/start">
                Continue with Google
              </a>
            ) : null}
            <label>
              Work email
              <input
                type="email"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                placeholder="you@company.com"
              />
            </label>
            <label>
              Organization name <span className="muted">(optional)</span>
              <input
                value={orgName}
                onChange={(e) => setOrgName(e.target.value)}
                placeholder="Acme Corp"
              />
            </label>
            <button type="button" className="gs-primary" disabled={busy || !email.trim()} onClick={signup}>
              {busy ? "Creating workspace…" : "Create workspace"}
            </button>
            {err ? <p className="error-text">{err}</p> : null}
          </div>
        )}
      </div>

      {!session && demoHint ? (
        <p className="muted gs-demo-hint">{demoHint} You are viewing the shared demo org until you sign in.</p>
      ) : null}

      {session && guide ? (
        <div className="gs-steps gs-onboarding">
          <h3>Onboarding checklist</h3>
          <ol>
            {guide.steps.map((step) => (
              <li key={step.id} className={step.done ? "done" : ""}>
                <strong>{step.title}</strong>
                <p className="muted">{step.detail}</p>
              </li>
            ))}
          </ol>
          <div className="gs-key-block">
            <h4>Ingest API key</h4>
            <p className="muted">Send as <code>DD-API-KEY</code> or <code>THINE-API-KEY</code> on every agent and OTLP client.</p>
            <code className="gs-key">{guide.ingest_api_key}</code>
            <button type="button" className="ghost" onClick={() => copyKey(guide.ingest_api_key)}>
              Copy key
            </button>
          </div>
          {ingestCurl ? (
            <div className="gs-key-block">
              <h4>Send a test metric</h4>
              <pre className="gs-curl">{ingestCurl}</pre>
            </div>
          ) : null}
          <div className="gs-cta-row">
            <button type="button" className="gs-primary" onClick={onInstallAgent}>
              Install agent →
            </button>
            <button type="button" className="ghost" onClick={onOpenDashboards}>
              Open dashboards
            </button>
          </div>
          <p className="muted">
            {agents > 0
              ? `${agents} agent(s) reporting in your org.`
              : "No agents yet — install one to see live host metrics."}
          </p>
        </div>
      ) : null}

      <div className="gs-steps">
        <h3>Typical rollout</h3>
        <ol>
          <li>
            <strong>Authenticate</strong> — Google OAuth or email signup provisions org + ingest key
          </li>
          <li>
            <strong>Instrument</strong> — OTLP/JSON ingest tags all series with your <code>org_id</code>
          </li>
          <li>
            <strong>Explore</strong> — Metrics Explorer and APM only show data for your org
          </li>
          <li>
            <strong>Operate</strong> — Dashboards, monitors, notebooks scoped to your workspace
          </li>
        </ol>
      </div>
    </section>
  );
}
