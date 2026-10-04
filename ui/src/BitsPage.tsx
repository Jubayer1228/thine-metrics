import { useEffect, useState } from "react";
import { api, type BitsAction, type BitsReply } from "./api";

const PROMPTS = [
  "Run the monitoring tutorial",
  "Install an agent",
  "Create a dashboard with golden signals",
  "Create a high CPU monitor with recovery",
  "How do I avoid alert fatigue?",
  "Why is API latency high?",
  "Open an RCA notebook",
  "Show fleet status",
  "Which SLOs are breaching?",
];

type NavTarget =
  | { page: "fleet" }
  | { page: "dashboards"; boardId?: string }
  | { page: "monitors" }
  | { page: "get-started" }
  | { page: "observability" }
  | { page: "notebooks"; notebookId?: string };

type Props = {
  onNavigate?: (target: NavTarget) => void;
  onBoardsChanged?: () => void;
  initialPrompt?: string | null;
};

export function BitsPage({ onNavigate, onBoardsChanged, initialPrompt }: Props) {
  const [message, setMessage] = useState(initialPrompt || PROMPTS[0]);
  const [history, setHistory] = useState<{ q: string; a: BitsReply }[]>([]);
  const [reply, setReply] = useState<BitsReply | null>(null);
  const [tools, setTools] = useState<{ name: string; description: string }[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<string | null>(null);

  useEffect(() => {
    api.mcpTools().then(setTools).catch(() => setTools([]));
  }, []);

  useEffect(() => {
    if (initialPrompt) setMessage(initialPrompt);
  }, [initialPrompt]);

  async function runActions(actions: BitsAction[] | undefined) {
    if (!actions?.length) return;
    for (const a of actions) {
      if (a.type === "bootstrap_agent") {
        try {
          const r = await api.fleetBootstrap({
            platform: a.platform || "linux",
            id: typeof a.id === "string" ? a.id : undefined,
            host: a.platform ? `${a.platform}-bits` : undefined,
          });
          setStatus(`Live agent reporting: ${r.agent.id}`);
        } catch (e) {
          setStatus(e instanceof Error ? e.message : "Bootstrap follow-up failed");
        }
      }
      if (a.type === "navigate" && a.page && onNavigate) {
        if (a.page === "dashboards") {
          onNavigate({ page: "dashboards", boardId: a.board_id });
        } else if (a.page === "fleet") {
          onNavigate({ page: "fleet" });
        } else if (a.page === "monitors") {
          onNavigate({ page: "monitors" });
        } else if (a.page === "get-started") {
          onNavigate({ page: "get-started" });
        } else if (a.page === "notebooks") {
          onNavigate({ page: "notebooks", notebookId: a.notebook_id });
        } else if (a.page === "observability") {
          onNavigate({ page: "observability" });
        }
      }
    }
    onBoardsChanged?.();
  }

  async function ask(q?: string) {
    const text = (q ?? message).trim();
    if (!text) return;
    setBusy(true);
    setError(null);
    setStatus(null);
    try {
      const a = await api.bitsChat(text);
      setReply(a);
      setHistory((h) => [...h, { q: text, a }].slice(-12));
      await runActions(a.actions);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Bits chat failed");
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="dd-panel bits-page">
      <div className="bits-hero">
        <div>
          <h2>Bits AI</h2>
          <p className="muted">
            Agentic assistant for the Datadog tutorial path — install agents, scaffold Golden Signal
            dashboards, publish monitors with recovery thresholds, and investigate with live citations.
          </p>
        </div>
        <div className="bits-tools">
          {tools.map((t) => (
            <code key={t.name} title={t.description}>
              {t.name}
            </code>
          ))}
        </div>
      </div>

      <div className="bits-prompts">
        {PROMPTS.map((p) => (
          <button
            key={p}
            type="button"
            className="ghost"
            onClick={() => {
              setMessage(p);
              void ask(p);
            }}
          >
            {p}
          </button>
        ))}
      </div>

      <div className="bits-input">
        <input
          value={message}
          onChange={(e) => setMessage(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && void ask()}
          placeholder='Try “Run the monitoring tutorial”'
        />
        <button type="button" disabled={busy} onClick={() => void ask()}>
          {busy ? "Working…" : "Ask Bits"}
        </button>
      </div>

      {error && (
        <div className="dd-banner compact">
          <span>{error}</span>
        </div>
      )}
      {status && (
        <div className="dd-banner compact">
          <span>{status}</span>
        </div>
      )}

      {reply && (
        <article className="bits-reply-card">
          <header>
            <strong>{reply.model ?? "bits"}</strong>
            {reply.intent && <span className="muted"> · {reply.intent}</span>}
          </header>
          <pre className="bits-reply-body">{reply.reply}</pre>
          {(reply.actions?.length ?? 0) > 0 && (
            <div className="bits-actions">
              <span className="muted">Actions</span>
              {reply.actions!.map((a, i) => (
                <code key={`${a.type}-${i}`}>
                  {a.type}
                  {a.page ? ` → ${a.page}` : ""}
                  {a.board_id ? ` (${a.board_id.slice(0, 8)}…)` : ""}
                  {a.notebook_id ? ` (nb ${a.notebook_id.slice(0, 8)}…)` : ""}
                  {a.platform ? ` [${a.platform}]` : ""}
                </code>
              ))}
            </div>
          )}
          {(reply.citations?.length ?? 0) > 0 && (
            <div className="bits-cites">
              {reply.citations!.map((c) => (
                <code key={c}>{c}</code>
              ))}
            </div>
          )}
          {reply.artifacts && (
            <details className="bits-artifacts">
              <summary>Artifacts</summary>
              <pre>{JSON.stringify(reply.artifacts, null, 2)}</pre>
            </details>
          )}
        </article>
      )}

      {history.length > 1 && (
        <div className="bits-history">
          <h3>Session</h3>
          {history
            .slice()
            .reverse()
            .map((h, i) => (
              <div key={`${h.q}-${i}`} className="bits-history-item">
                <strong>You:</strong> {h.q}
                <div className="muted">
                  Bits · {h.a.intent} — {h.a.reply.slice(0, 140)}
                  {h.a.reply.length > 140 ? "…" : ""}
                </div>
              </div>
            ))}
        </div>
      )}
    </section>
  );
}
