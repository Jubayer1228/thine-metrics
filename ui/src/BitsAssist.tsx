import { useEffect, useState } from "react";
import { api, type BitsReply } from "./api";

const PROMPTS = [
  "Run the monitoring tutorial",
  "Install an agent",
  "Create a dashboard with golden signals",
  "Create a high CPU monitor with recovery",
  "Why is API latency high?",
  "Which SLOs are breaching?",
];

export function BitsAssist() {
  const [message, setMessage] = useState(PROMPTS[0]);
  const [reply, setReply] = useState<BitsReply | null>(null);
  const [tools, setTools] = useState<{ name: string; description: string }[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.mcpTools().then(setTools).catch(() => setTools([]));
  }, []);

  async function ask(q?: string) {
    const text = (q ?? message).trim();
    if (!text) return;
    setBusy(true);
    setError(null);
    try {
      setReply(await api.bitsChat(text));
    } catch (e) {
      setError(e instanceof Error ? e.message : "Bits chat failed");
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="bits-assist obs-card">
      <header className="bits-head">
        <div>
          <h3>Bits · agentic RCA</h3>
          <p className="muted">
            Agentic — can run the Datadog tutorial (agent + Golden Signals board + monitors). Open Bits AI in the nav for the full console.
          </p>
        </div>
        <div className="bits-tools">
          {tools.map((t) => (
            <code key={t.name} title={t.description}>
              {t.name}
            </code>
          ))}
        </div>
      </header>
      <div className="bits-prompts">
        {PROMPTS.map((p) => (
          <button key={p} type="button" className="ghost" onClick={() => { setMessage(p); void ask(p); }}>
            {p}
          </button>
        ))}
      </div>
      <div className="bits-input">
        <input value={message} onChange={(e) => setMessage(e.target.value)} onKeyDown={(e) => e.key === "Enter" && void ask()} />
        <button type="button" disabled={busy} onClick={() => void ask()}>
          {busy ? "Thinking…" : "Ask"}
        </button>
      </div>
      {error && <p className="muted">{error}</p>}
      {reply && (
        <div className="bits-reply">
          <strong>{reply.model ?? "bits"}</strong>
          {reply.intent && <span className="muted"> · {reply.intent}</span>}
          <p>{reply.reply}</p>
          {(reply.citations?.length ?? 0) > 0 && (
            <div className="bits-cites">
              {reply.citations!.map((c) => (
                <code key={c}>{c}</code>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
