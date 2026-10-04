import { useEffect, useMemo, useState } from "react";
import { api, type KubeContainerMap, type KubeMapNode } from "./api";

type Fill = "CPU" | "Memory" | "Restarts";

function heat(pct: number): string {
  const t = Math.max(0, Math.min(100, pct)) / 100;
  if (t < 0.4) return `rgba(46, 196, 182, ${0.25 + t})`;
  if (t < 0.7) return `rgba(244, 162, 97, ${0.3 + t * 0.4})`;
  return `rgba(242, 95, 92, ${0.35 + t * 0.4})`;
}

function fillValue(n: KubeMapNode, fill: Fill): number {
  if (fill === "Memory") return n.mem_pct;
  if (fill === "Restarts") return Math.min(100, n.restarts * 20);
  return n.cpu_pct;
}

/** Datadog Container Map — kube hierarchy (ns → deploy → pod → container), not Host Map. */
export function ContainerMap() {
  const [data, setData] = useState<KubeContainerMap | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [fill, setFill] = useState<Fill>("CPU");
  const [nsFilter, setNsFilter] = useState<string>("all");
  const [selected, setSelected] = useState<KubeMapNode | null>(null);

  useEffect(() => {
    let cancelled = false;
    async function load() {
      try {
        const m = await api.kubeContainerMap();
        if (!cancelled) {
          setData(m);
          setError(null);
        }
      } catch (e) {
        if (!cancelled) setError(e instanceof Error ? e.message : "Container map failed");
      }
    }
    void load();
    const id = setInterval(() => void load(), 12_000);
    return () => {
      cancelled = true;
      clearInterval(id);
    };
  }, []);

  const namespaces = useMemo(
    () => (data?.nodes ?? []).filter((n) => n.kind === "namespace"),
    [data],
  );

  const tree = useMemo(() => {
    const nodes = data?.nodes ?? [];
    const byParent = new Map<string | null, KubeMapNode[]>();
    for (const n of nodes) {
      if (nsFilter !== "all" && n.kind !== "namespace") {
        // keep if under selected namespace
        const walk = (id: string | null | undefined): boolean => {
          if (!id) return false;
          if (id === `ns:${nsFilter}`) return true;
          const parent = nodes.find((x) => x.id === id)?.parent;
          return walk(parent);
        };
        if (n.kind === "deployment" && n.parent !== `ns:${nsFilter}`) continue;
        if (n.kind !== "deployment" && !walk(n.parent)) continue;
      }
      if (n.kind === "namespace" && nsFilter !== "all" && n.name !== nsFilter) continue;
      const key = n.parent ?? null;
      const arr = byParent.get(key) ?? [];
      arr.push(n);
      byParent.set(key, arr);
    }
    return byParent;
  }, [data, nsFilter]);

  const nsNodes = tree.get(null) ?? namespaces;

  return (
    <div className="obs-section">
      <div className="corr-hero" style={{ marginBottom: "0.75rem" }}>
        <div>
          <h2 style={{ margin: 0 }}>Container Map</h2>
          <p className="muted" style={{ margin: "0.25rem 0 0" }}>
            Kubernetes topology: namespace → deployment → pod → container — Datadog Container Map, not Host Map.
          </p>
        </div>
        <div style={{ display: "flex", gap: "0.5rem", flexWrap: "wrap", alignItems: "center" }}>
          <select value={nsFilter} onChange={(e) => setNsFilter(e.target.value)}>
            <option value="all">All namespaces</option>
            {namespaces.map((n) => (
              <option key={n.id} value={n.name}>
                {n.name}
              </option>
            ))}
          </select>
          <select value={fill} onChange={(e) => setFill(e.target.value as Fill)}>
            {(data?.fill_by ?? ["CPU", "Memory", "Restarts"]).map((f) => (
              <option key={f} value={f}>
                Fill: {f}
              </option>
            ))}
          </select>
        </div>
      </div>
      {error && <p className="error">{error}</p>}
      <div className="obs-kpi-row">
        <div className="obs-kpi">
          <span>Namespaces</span>
          <strong>{data?.counts.namespaces ?? "—"}</strong>
        </div>
        <div className="obs-kpi">
          <span>Deployments</span>
          <strong>{data?.counts.deployments ?? "—"}</strong>
        </div>
        <div className="obs-kpi">
          <span>Containers</span>
          <strong>{data?.counts.containers ?? "—"}</strong>
        </div>
      </div>
      <div className="kube-map" style={{ display: "grid", gap: "1rem" }}>
        {nsNodes.map((ns) => {
          const deps = tree.get(ns.id) ?? [];
          return (
            <div key={ns.id} className="obs-card">
              <header style={{ display: "flex", justifyContent: "space-between", marginBottom: "0.5rem" }}>
                <strong>
                  <code>{ns.name}</code> <span className="muted">namespace</span>
                </strong>
                <span className="muted">
                  CPU {ns.cpu_pct.toFixed(0)}% · Mem {ns.mem_pct.toFixed(0)}%
                </span>
              </header>
              <div style={{ display: "flex", flexWrap: "wrap", gap: "0.75rem" }}>
                {deps.map((dep) => {
                  const pods = tree.get(dep.id) ?? [];
                  return (
                    <div
                      key={dep.id}
                      style={{
                        minWidth: 160,
                        padding: "0.5rem",
                        borderRadius: 8,
                        background: heat(fillValue(dep, fill)),
                        border: "1px solid var(--surf-border, rgba(255,255,255,0.08))",
                      }}
                    >
                      <div style={{ fontWeight: 600, marginBottom: 6 }}>{dep.name}</div>
                      <div style={{ display: "flex", flexWrap: "wrap", gap: 4 }}>
                        {pods.map((pod) => {
                          const ctrs = tree.get(pod.id) ?? [];
                          return (
                            <button
                              key={pod.id}
                              type="button"
                              title={`${pod.name} · ${ctrs.length} ctr`}
                              onClick={() => setSelected(pod)}
                              style={{
                                width: 28 + Math.min(ctrs.length, 4) * 6,
                                height: 28 + Math.min(ctrs.length, 4) * 4,
                                borderRadius: 6,
                                border: selected?.id === pod.id ? "2px solid #5B91EB" : "1px solid transparent",
                                background: heat(fillValue(pod, fill)),
                                cursor: "pointer",
                              }}
                            />
                          );
                        })}
                      </div>
                      {!pods.length && <span className="muted">No pods</span>}
                    </div>
                  );
                })}
                {!deps.length && <p className="muted">No deployments in this namespace</p>}
              </div>
            </div>
          );
        })}
        {!nsNodes.length && <p className="muted">No kube topology yet</p>}
      </div>
      {selected && (
        <aside className="obs-card" style={{ marginTop: "0.75rem" }}>
          <h3>
            {selected.kind}: {selected.name}
          </h3>
          <p className="muted">
            service <code>{selected.service || "—"}</code> · env <code>{selected.env || "—"}</code> ·{" "}
            {selected.status} · restarts {selected.restarts}
          </p>
          <p>
            CPU {selected.cpu_pct.toFixed(1)}% · Memory {selected.mem_pct.toFixed(1)}%
          </p>
          <ul className="corr-metric-list">
            {(tree.get(selected.id) ?? []).map((c) => (
              <li key={c.id}>
                <code>{c.name}</code>
                <span>
                  {c.cpu_pct.toFixed(0)}% cpu · {c.restarts} rst
                </span>
              </li>
            ))}
          </ul>
        </aside>
      )}
    </div>
  );
}
