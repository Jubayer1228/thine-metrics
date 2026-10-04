import { useEffect, useMemo, useState, type ReactNode } from "react";
import { api, type DashboardList } from "./api";

export type AppPage =
  | "dashboards"
  | "observability"
  | "compare"
  | "explorer"
  | "summary"
  | "correlations"
  | "monitors"
  | "docs"
  | "notebooks"
  | "integrations"
  | "get-started"
  | "fleet"
  | "bits";

export type ObsDeepLink =
  | "overview"
  | "cloudcraft"
  | "usm"
  | "apm"
  | "apm-traces"
  | "apm-map"
  | "apm-catalog"
  | "container-map"
  | "dbm"
  | "ai"
  | "infra"
  | "hostmap"
  | "containers"
  | "infra-envs"
  | "infra-k8s"
  | "processes"
  | "serverless"
  | "gpu"
  | "logs"
  | "logs-live"
  | "logs-errors"
  | "logs-sds"
  | "logs-patterns"
  | "events"
  | "ux"
  | "data"
  | "cost"
  | "ha"
  | "ha-watchdog"
  | "slos";

/** Normalize fine-grained deep links to the Observability panel they belong to. */
export function obsBaseTab(tab: ObsDeepLink): ObsDeepLink {
  if (tab === "apm-traces" || tab === "apm-map" || tab === "apm-catalog") return "apm";
  if (tab === "logs-live" || tab === "logs-errors" || tab === "logs-sds" || tab === "logs-patterns") return "logs";
  if (tab === "container-map") return "container-map";
  if (tab === "ha-watchdog") return "ha";
  return tab;
}

export type DashNavAction =
  | { type: "open-board" }
  | { type: "open-list"; listId?: string | null; preset?: "all" | "deleted" }
  | { type: "open-guide" }
  | { type: "new-dashboard" };

type NavId =
  | "watchdog"
  | "events"
  | "bits"
  | "dashboards"
  | "infrastructure"
  | "monitors"
  | "metrics"
  | "integrations"
  | "apm"
  | "notebooks"
  | "logs"
  | "security"
  | "ai"
  | "ux"
  | "observability";

type Props = {
  page: AppPage;
  connected: boolean;
  seriesLabel: string;
  boardCount: number;
  onPage: (page: AppPage) => void;
  onObsTab: (tab: ObsDeepLink) => void;
  onDashAction: (action: DashNavAction) => void;
  onBitsPrompt?: (prompt: string) => void;
  activeDashListId?: string | null;
  obsTab: ObsDeepLink | null;
};

type NavItem = {
  id: NavId;
  label: string;
  chevron?: boolean;
  page?: AppPage;
  obsTab?: ObsDeepLink;
  icon: ReactNode;
};

function Icon({ d, children }: { d?: string; children?: ReactNode }) {
  return (
    <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
      {d ? <path d={d} /> : children}
    </svg>
  );
}

const ICONS = {
  watchdog: (
    <Icon>
      <path d="M12 3l2.2 4.5 5 .7-3.6 3.5.9 5.1L12 14.8 7.5 16.8l.9-5.1L4.8 8.2l5-.7L12 3z" />
    </Icon>
  ),
  events: (
    <Icon>
      <path d="M8 6h13M8 12h13M8 18h13M3 6h.01M3 12h.01M3 18h.01" />
    </Icon>
  ),
  dashboards: (
    <Icon>
      <rect x="3" y="3" width="8" height="8" rx="1" />
      <rect x="13" y="3" width="8" height="5" rx="1" />
      <rect x="13" y="10" width="8" height="11" rx="1" />
      <rect x="3" y="13" width="8" height="8" rx="1" />
    </Icon>
  ),
  infrastructure: (
    <Icon>
      <rect x="3" y="4" width="18" height="5" rx="1" />
      <rect x="3" y="10" width="18" height="5" rx="1" />
      <rect x="3" y="16" width="18" height="4" rx="1" />
      <circle cx="7" cy="6.5" r="0.8" fill="currentColor" stroke="none" />
      <circle cx="7" cy="12.5" r="0.8" fill="currentColor" stroke="none" />
    </Icon>
  ),
  monitors: (
    <Icon>
      <path d="M4 12c2-4 4-6 8-6s6 2 8 6c-2 4-4 6-8 6s-6-2-8-6z" />
      <circle cx="12" cy="12" r="2.2" />
    </Icon>
  ),
  metrics: (
    <Icon>
      <path d="M4 18V6M10 18V10M16 18V8M22 18V4" />
    </Icon>
  ),
  integrations: (
    <Icon>
      <path d="M9 7V4h6v3M9 17v3h6v-3M7 9H4v6h3M17 9h3v6h-3M9 9h6v6H9z" />
    </Icon>
  ),
  apm: (
    <Icon>
      <path d="M3 17l5-5 4 3 8-9" />
      <path d="M15 6h5v5" />
    </Icon>
  ),
  notebooks: (
    <Icon>
      <path d="M5 4h11a2 2 0 012 2v14l-4-2-4 2-4-2-4 2V6a2 2 0 012-2z" />
      <path d="M9 8h6M9 12h6" />
    </Icon>
  ),
  logs: (
    <Icon>
      <path d="M6 3h9l3 3v15H6z" />
      <path d="M9 10h6M9 14h6M9 18h3" />
    </Icon>
  ),
  security: (
    <Icon>
      <path d="M12 3l8 3v6c0 5-3.5 8-8 9-4.5-1-8-4-8-9V6l8-3z" />
    </Icon>
  ),
  ai: (
    <Icon>
      <circle cx="12" cy="12" r="3" />
      <path d="M12 3v3M12 18v3M3 12h3M18 12h3M5.6 5.6l2.1 2.1M16.3 16.3l2.1 2.1M18.4 5.6l-2.1 2.1M7.7 16.3l-2.1 2.1" />
    </Icon>
  ),
  ux: (
    <Icon>
      <circle cx="12" cy="8" r="3.2" />
      <path d="M5 19c1.5-3.5 4-5 7-5s5.5 1.5 7 5" />
    </Icon>
  ),
  observability: (
    <Icon>
      <circle cx="12" cy="12" r="8" />
      <circle cx="12" cy="12" r="3" />
    </Icon>
  ),
  bits: (
    <Icon>
      <path d="M12 3l1.8 4.2L18 9l-4.2 1.8L12 15l-1.8-4.2L6 9l4.2-1.8L12 3z" />
      <path d="M5 16l1 2.2L8.5 19 6 19.8 5 22l-1-2.2L1.5 19 4 18.2 5 16z" />
      <path d="M18 14l.8 1.8L20.5 17l-1.7.7L18 19.5l-.8-1.8L15.5 17l1.7-.7L18 14z" />
    </Icon>
  ),
  help: (
    <Icon>
      <circle cx="12" cy="12" r="9" />
      <path d="M9.5 9a2.5 2.5 0 114 2c-.8.6-1.5 1.2-1.5 2.2V14" />
      <circle cx="12" cy="17" r="0.7" fill="currentColor" stroke="none" />
    </Icon>
  ),
};

const PRIMARY: NavItem[] = [
  { id: "watchdog", label: "Watchdog", icon: ICONS.watchdog, chevron: true, page: "observability", obsTab: "ha" },
  { id: "events", label: "Events", icon: ICONS.events, chevron: true, page: "observability", obsTab: "events" },
  { id: "bits", label: "Bits AI", icon: ICONS.bits, chevron: true, page: "bits" },
  { id: "dashboards", label: "Dashboards", icon: ICONS.dashboards, chevron: true, page: "dashboards" },
  { id: "infrastructure", label: "Infrastructure", icon: ICONS.infrastructure, chevron: true, page: "observability", obsTab: "infra" },
  { id: "monitors", label: "Monitors", icon: ICONS.monitors, chevron: true, page: "monitors" },
  { id: "metrics", label: "Metrics", icon: ICONS.metrics, chevron: true, page: "explorer" },
  { id: "integrations", label: "Integrations", icon: ICONS.integrations, chevron: true, page: "integrations" },
  { id: "apm", label: "APM", icon: ICONS.apm, chevron: true, page: "observability", obsTab: "apm" },
  { id: "notebooks", label: "Notebooks", icon: ICONS.notebooks, chevron: true, page: "notebooks" },
  { id: "logs", label: "Logs", icon: ICONS.logs, chevron: true, page: "observability", obsTab: "logs" },
  { id: "security", label: "Security", icon: ICONS.security, chevron: true, page: "observability", obsTab: "usm" },
  { id: "ai", label: "AI / Agents", icon: ICONS.ai, chevron: true, page: "observability", obsTab: "ai" },
  { id: "ux", label: "UX Monitoring", icon: ICONS.ux, chevron: true, page: "observability", obsTab: "ux" },
  { id: "observability", label: "Observability", icon: ICONS.observability, chevron: true, page: "observability", obsTab: "overview" },
];

function activeNavId(page: AppPage, obsTab: ObsDeepLink | null): NavId {
  if (page === "bits") return "bits";
  if (page === "dashboards" || page === "get-started") return "dashboards";
  if (page === "monitors") return "monitors";
  if (page === "explorer" || page === "summary" || page === "correlations") return "metrics";
  if (page === "integrations" || page === "compare") return "integrations";
  if (page === "notebooks") return "notebooks";
  if (page === "fleet") return "infrastructure";
  if (page === "observability") {
    if (obsTab === "ha" || obsTab === "ha-watchdog") return "watchdog";
    if (obsTab === "events") return "events";
    if (obsTab === "logs" || obsTab === "logs-live" || obsTab === "logs-errors" || obsTab === "logs-sds" || obsTab === "logs-patterns")
      return "logs";
    if (obsTab === "apm" || obsTab === "apm-traces" || obsTab === "apm-map" || obsTab === "apm-catalog") return "apm";
    if (obsTab === "ux") return "ux";
    if (
      obsTab === "infra" ||
      obsTab === "hostmap" ||
      obsTab === "containers" ||
      obsTab === "container-map" ||
      obsTab === "infra-envs" ||
      obsTab === "infra-k8s" ||
      obsTab === "processes" ||
      obsTab === "serverless" ||
      obsTab === "gpu" ||
      obsTab === "dbm" ||
      obsTab === "cloudcraft"
    ) {
      return "infrastructure";
    }
    if (obsTab === "usm") return "security";
    if (obsTab === "ai") return "ai";
    if (obsTab === "slos") return "monitors";
    return "observability";
  }
  return "dashboards";
}

const EXPANDABLE: NavId[] = [
  "dashboards",
  "metrics",
  "monitors",
  "integrations",
  "notebooks",
  "bits",
  "infrastructure",
  "observability",
  "apm",
  "logs",
  "security",
  "ai",
  "watchdog",
  "ux",
  "events",
];

export function SideNav({
  page,
  connected,
  seriesLabel,
  boardCount,
  onPage,
  onObsTab,
  onDashAction,
  onBitsPrompt,
  activeDashListId,
  obsTab,
}: Props) {
  const [expanded, setExpanded] = useState<NavId | null>("dashboards");
  const [lists, setLists] = useState<DashboardList[]>([]);
  const [dashSearch, setDashSearch] = useState("");
  const [listFilter, setListFilter] = useState("");
  const [favorites, setFavorites] = useState<Set<string>>(() => new Set(["preset-all"]));

  const active = page === "docs" ? null : activeNavId(page, obsTab);

  useEffect(() => {
    if (page === "dashboards" || expanded === "dashboards") {
      api.boardLists().then(setLists).catch(() => setLists([]));
    }
  }, [page, expanded]);

  useEffect(() => {
    if (!active) return;
    if (EXPANDABLE.includes(active)) setExpanded(active);
  }, [active]);

  function goObs(tab: ObsDeepLink) {
    onPage("observability");
    onObsTab(tab);
  }

  function SubRow({
    label,
    on,
    onClick,
  }: {
    label: string;
    on?: boolean;
    onClick: () => void;
  }) {
    return (
      <button type="button" className={`dd-subnav-row${on ? " on" : ""}`} onClick={onClick}>
        <span>{label}</span>
      </button>
    );
  }

  const showSecondary = expanded != null;

  const filteredLists = useMemo(() => {
    const q = listFilter.trim().toLowerCase();
    if (!q) return lists;
    return lists.filter((l) => l.name.toLowerCase().includes(q));
  }, [lists, listFilter]);

  function go(item: NavItem) {
    setExpanded(item.id);
    if (item.page) onPage(item.page);
    if (item.obsTab) onObsTab(item.obsTab);
    if (item.id === "dashboards") onDashAction({ type: "open-list", preset: "all" });
  }

  function toggleFavorite(id: string) {
    setFavorites((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  return (
    <div className={`dd-shell-nav${showSecondary ? " with-secondary" : ""}`}>
      <aside className="dd-rail">
        <div className="dd-rail-logo" title="Thine Metrics">
          <span className="dd-mark">T</span>
          <strong>THINE</strong>
        </div>
        <nav className="dd-rail-nav">
          {PRIMARY.map((item) => (
            <button
              key={item.id}
              type="button"
              className={`dd-rail-item${active === item.id ? " active" : ""}`}
              onClick={() => go(item)}
              title={item.label}
            >
              <span className="dd-rail-ico">{item.icon}</span>
              <span className="dd-rail-label">{item.label}</span>
              {item.chevron && <span className="dd-rail-chevron">›</span>}
            </button>
          ))}
        </nav>
        <div className="dd-rail-foot">
          <button
            type="button"
            className={`dd-rail-item${page === "docs" ? " active" : ""}`}
            onClick={() => {
              setExpanded(null);
              onPage("docs");
            }}
            title="Help"
          >
            <span className="dd-rail-ico">{ICONS.help}</span>
            <span className="dd-rail-label">Help</span>
          </button>
          <div className="dd-rail-status">
            <span className={`dot ${connected ? "ok" : "bad"}`} />
            <span>{connected ? "Connected" : "Offline"}</span>
            <small>{seriesLabel}</small>
          </div>
        </div>
      </aside>

      {showSecondary && (
        <aside className="dd-subnav">
          {expanded === "dashboards" && (
            <>
              <div className="dd-subnav-search">
                <span className="dd-subnav-search-ico" aria-hidden>
                  ⌕
                </span>
                <input
                  placeholder="Search dashboards"
                  value={dashSearch}
                  onChange={(e) => setDashSearch(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") {
                      onPage("dashboards");
                      onDashAction({ type: "open-list", preset: "all" });
                    }
                  }}
                />
              </div>
              <div className="dd-subnav-actions">
                <button type="button" className="dd-subnav-btn" onClick={() => onDashAction({ type: "new-dashboard" })}>
                  New Dashboard +
                </button>
              </div>
              <div className="dd-subnav-section">
                <div className="dd-subnav-head">
                  <span>Lists</span>
                  <input placeholder="Filter lists" value={listFilter} onChange={(e) => setListFilter(e.target.value)} />
                </div>
              </div>
              <div className="dd-subnav-section">
                <h4>Favorite Lists</h4>
                <button
                  type="button"
                  className={`dd-subnav-row${activeDashListId == null && page === "dashboards" ? " on" : ""}`}
                  onClick={() => {
                    onPage("dashboards");
                    onDashAction({ type: "open-list", listId: null, preset: "all" });
                  }}
                >
                  <span
                    className="star on"
                    onClick={(e) => {
                      e.stopPropagation();
                      toggleFavorite("preset-all");
                    }}
                  >
                    ★
                  </span>
                  <span>All Dashboards</span>
                  <em>{boardCount}</em>
                </button>
              </div>
              <div className="dd-subnav-section">
                <h4>Preset Lists</h4>
                {(
                  [
                    { id: "all", label: "All Custom", preset: "all" as const },
                    { id: "hosts", label: "All Hosts", preset: "all" as const },
                    { id: "created", label: "Created By You", preset: "all" as const },
                    { id: "freq", label: "Frequently Viewed By You", preset: "all" as const },
                    { id: "deleted", label: "Recently Deleted", preset: "deleted" as const },
                  ] as const
                ).map((p) => (
                  <button
                    key={p.id}
                    type="button"
                    className="dd-subnav-row"
                    onClick={() => {
                      onPage("dashboards");
                      onDashAction({ type: "open-list", listId: null, preset: p.preset });
                    }}
                  >
                    <span
                      className={`star${favorites.has(p.id) ? " on" : ""}`}
                      onClick={(e) => {
                        e.stopPropagation();
                        toggleFavorite(p.id);
                      }}
                    >
                      {favorites.has(p.id) ? "★" : "☆"}
                    </span>
                    <span>{p.label}</span>
                  </button>
                ))}
              </div>
              <div className="dd-subnav-section grow">
                <h4>Shared, Editable Lists</h4>
                {filteredLists.map((l) => (
                  <button
                    key={l.id}
                    type="button"
                    className={`dd-subnav-row${activeDashListId === l.id ? " on" : ""}`}
                    onClick={() => {
                      onPage("dashboards");
                      onDashAction({ type: "open-list", listId: l.id, preset: "all" });
                    }}
                  >
                    <span
                      className={`star${favorites.has(l.id) ? " on" : ""}`}
                      onClick={(e) => {
                        e.stopPropagation();
                        toggleFavorite(l.id);
                      }}
                    >
                      {favorites.has(l.id) ? "★" : "☆"}
                    </span>
                    <span>{l.name}</span>
                    <em>{l.board_ids?.length ?? 0}</em>
                  </button>
                ))}
                {!filteredLists.length && <p className="dd-subnav-empty">No lists</p>}
              </div>
              <div className="dd-subnav-section">
                <button type="button" className="dd-subnav-link" onClick={() => onPage("get-started")}>
                  Welcome / Get Started
                </button>
                <button type="button" className="dd-subnav-link" onClick={() => onDashAction({ type: "open-board" })}>
                  Open active board
                </button>
                <button type="button" className="dd-subnav-link" onClick={() => onDashAction({ type: "open-guide" })}>
                  Configure / Guide
                </button>
              </div>
              {dashSearch.trim() && (
                <p className="dd-subnav-hint">Press Enter to open Dashboard List for “{dashSearch}”</p>
              )}
            </>
          )}

          {expanded === "metrics" && (
            <>
              <h3 className="dd-subnav-title">Metrics</h3>
              <button type="button" className={`dd-subnav-row${page === "explorer" ? " on" : ""}`} onClick={() => onPage("explorer")}>
                <span>Metrics Explorer</span>
              </button>
              <button type="button" className={`dd-subnav-row${page === "summary" ? " on" : ""}`} onClick={() => onPage("summary")}>
                <span>Metrics Summary</span>
              </button>
              <button
                type="button"
                className={`dd-subnav-row${page === "correlations" ? " on" : ""}`}
                onClick={() => onPage("correlations")}
              >
                <span>Metric correlations</span>
              </button>
            </>
          )}

          {expanded === "watchdog" && (
            <>
              <h3 className="dd-subnav-title">Watchdog</h3>
              <SubRow label="Anomalies" on={obsTab === "ha-watchdog" || obsTab === "ha"} onClick={() => goObs("ha-watchdog")} />
              <SubRow label="HA status" on={obsTab === "ha"} onClick={() => goObs("ha")} />
              <SubRow label="Ask Bits to investigate" on={page === "bits"} onClick={() => onPage("bits")} />
            </>
          )}

          {expanded === "events" && (
            <>
              <h3 className="dd-subnav-title">Events</h3>
              <SubRow label="Event Explorer" on={obsTab === "events"} onClick={() => goObs("events")} />
              <SubRow label="Alert events" on={page === "monitors"} onClick={() => onPage("monitors")} />
              <SubRow label="Deploy / change events" on={obsTab === "events"} onClick={() => goObs("events")} />
            </>
          )}

          {expanded === "bits" && (
            <>
              <h3 className="dd-subnav-title">Bits AI</h3>
              <SubRow
                label="Chat console"
                on={page === "bits"}
                onClick={() => {
                  onPage("bits");
                  onBitsPrompt?.("Run the monitoring tutorial");
                }}
              />
              <SubRow
                label="Run monitoring tutorial"
                on={page === "bits"}
                onClick={() => {
                  onPage("bits");
                  onBitsPrompt?.("Run the monitoring tutorial");
                }}
              />
              <SubRow
                label="Install agent (via Bits)"
                on={page === "bits"}
                onClick={() => {
                  onPage("bits");
                  onBitsPrompt?.("Install an agent");
                }}
              />
              <SubRow
                label="Golden Signals board"
                on={page === "bits"}
                onClick={() => {
                  onPage("bits");
                  onBitsPrompt?.("Create a dashboard with golden signals");
                }}
              />
              <SubRow
                label="Create CPU monitor"
                on={page === "bits"}
                onClick={() => {
                  onPage("bits");
                  onBitsPrompt?.("Create a high CPU monitor with recovery");
                }}
              />
            </>
          )}

          {expanded === "infrastructure" && (
            <>
              <h3 className="dd-subnav-title">Infrastructure</h3>
              <SubRow label="Welcome / Quick Start" on={page === "get-started"} onClick={() => onPage("get-started")} />
              <SubRow label="Fleet Automation" on={page === "fleet"} onClick={() => onPage("fleet")} />
              <SubRow label="Install Agents" on={page === "fleet"} onClick={() => onPage("fleet")} />
              <SubRow label="Host Map" on={obsTab === "hostmap"} onClick={() => goObs("hostmap")} />
              <SubRow
                label="Container Map"
                on={obsTab === "container-map"}
                onClick={() => goObs("container-map")}
              />
              <SubRow
                label="Containers Explorer"
                on={obsTab === "containers"}
                onClick={() => goObs("containers")}
              />
              <SubRow
                label="Environments (prod/staging/dev)"
                on={obsTab === "infra-envs"}
                onClick={() => goObs("infra-envs")}
              />
              <SubRow
                label="K8s resource utilization"
                on={obsTab === "infra-k8s"}
                onClick={() => goObs("infra-k8s")}
              />
              <SubRow label="Live Processes" on={obsTab === "processes"} onClick={() => goObs("processes")} />
              <SubRow label="Serverless" on={obsTab === "serverless"} onClick={() => goObs("serverless")} />
              <SubRow label="Overview" on={obsTab === "infra"} onClick={() => goObs("infra")} />
              <SubRow label="GPU" on={obsTab === "gpu"} onClick={() => goObs("gpu")} />
              <SubRow label="Databases" on={obsTab === "dbm"} onClick={() => goObs("dbm")} />
              <SubRow label="Cloudcraft" on={obsTab === "cloudcraft"} onClick={() => goObs("cloudcraft")} />
            </>
          )}

          {expanded === "monitors" && (
            <>
              <h3 className="dd-subnav-title">Monitors</h3>
              <SubRow label="Manage Monitors" on={page === "monitors"} onClick={() => onPage("monitors")} />
              <SubRow label="New Metric Monitor" on={page === "monitors"} onClick={() => onPage("monitors")} />
              <SubRow label="Service Level Objectives" on={obsTab === "slos"} onClick={() => goObs("slos")} />
              <SubRow label="Watchdog anomalies" on={obsTab === "ha-watchdog" || obsTab === "ha"} onClick={() => goObs("ha-watchdog")} />
              <SubRow label="Alert fatigue tips (Bits)" on={page === "bits"} onClick={() => onPage("bits")} />
            </>
          )}

          {expanded === "apm" && (
            <>
              <h3 className="dd-subnav-title">APM</h3>
              <SubRow label="Services" on={obsTab === "apm"} onClick={() => goObs("apm")} />
              <SubRow label="Software Catalog" on={obsTab === "apm-catalog"} onClick={() => goObs("apm-catalog")} />
              <SubRow label="Traces / Waterfall" on={obsTab === "apm-traces"} onClick={() => goObs("apm-traces")} />
              <SubRow label="Service Map" on={obsTab === "apm-map"} onClick={() => goObs("apm-map")} />
              <SubRow label="Database monitoring" on={obsTab === "dbm"} onClick={() => goObs("dbm")} />
            </>
          )}

          {expanded === "logs" && (
            <>
              <h3 className="dd-subnav-title">Logs</h3>
              <SubRow label="Log Explorer" on={obsTab === "logs"} onClick={() => goObs("logs")} />
              <SubRow label="Live Tail" on={obsTab === "logs-live"} onClick={() => goObs("logs-live")} />
              <SubRow label="Error Tracking" on={obsTab === "logs-errors"} onClick={() => goObs("logs-errors")} />
              <SubRow label="Sensitive Data Scanner" on={obsTab === "logs-sds"} onClick={() => goObs("logs-sds")} />
              <SubRow label="Log patterns / facets" on={obsTab === "logs-patterns"} onClick={() => goObs("logs-patterns")} />
            </>
          )}

          {expanded === "security" && (
            <>
              <h3 className="dd-subnav-title">Security</h3>
              <SubRow label="USM / eBPF" on={obsTab === "usm"} onClick={() => goObs("usm")} />
              <SubRow label="Network map" on={obsTab === "usm"} onClick={() => goObs("usm")} />
              <SubRow label="SDS / PII scan" on={obsTab === "logs-sds"} onClick={() => goObs("logs-sds")} />
              <SubRow label="Cloud Cost risk" on={obsTab === "cost"} onClick={() => goObs("cost")} />
            </>
          )}

          {expanded === "ai" && (
            <>
              <h3 className="dd-subnav-title">AI Observability</h3>
              <SubRow label="Projects & runs" on={obsTab === "ai"} onClick={() => goObs("ai")} />
              <SubRow label="Evals & graders" on={obsTab === "ai"} onClick={() => goObs("ai")} />
              <SubRow label="Datasets / feedback" on={obsTab === "ai"} onClick={() => goObs("ai")} />
              <SubRow label="Bits AI console" on={page === "bits"} onClick={() => onPage("bits")} />
            </>
          )}

          {expanded === "ux" && (
            <>
              <h3 className="dd-subnav-title">UX Monitoring</h3>
              <SubRow label="RUM sessions" on={obsTab === "ux"} onClick={() => goObs("ux")} />
              <SubRow label="Web Vitals" on={obsTab === "ux"} onClick={() => goObs("ux")} />
              <SubRow label="Synthetic tests" on={obsTab === "ux"} onClick={() => goObs("ux")} />
              <SubRow label="Frontend APM" on={obsTab === "apm"} onClick={() => goObs("apm")} />
            </>
          )}

          {expanded === "observability" && (
            <>
              <h3 className="dd-subnav-title">Observability</h3>
              {(
                [
                  ["overview", "Overview"],
                  ["cloudcraft", "Cloudcraft"],
                  ["usm", "USM / eBPF"],
                  ["apm", "APM"],
                  ["dbm", "Database"],
                  ["ai", "AI / Agents"],
                  ["infra", "Infra & Processes"],
                  ["gpu", "GPU"],
                  ["logs", "Logs & Errors"],
                  ["events", "Events"],
                  ["ux", "UX / RUM"],
                  ["data", "Streams & Lineage"],
                  ["cost", "Cloud Cost"],
                  ["slos", "SLOs"],
                  ["ha", "HA / Watchdog"],
                ] as [ObsDeepLink, string][]
              ).map(([id, label]) => (
                <SubRow
                  key={id}
                  label={label}
                  on={obsTab === id && page === "observability"}
                  onClick={() => goObs(id)}
                />
              ))}
            </>
          )}

          {expanded === "integrations" && (
            <>
              <h3 className="dd-subnav-title">Integrations</h3>
              <SubRow label="Catalog & marketplace" on={page === "integrations"} onClick={() => onPage("integrations")} />
              <SubRow label="Compare vendors" on={page === "compare"} onClick={() => onPage("compare")} />
              <SubRow label="Integration docs" on={page === "docs"} onClick={() => onPage("docs")} />
              <SubRow label="Fleet / Agents" on={page === "fleet"} onClick={() => onPage("fleet")} />
            </>
          )}

          {expanded === "notebooks" && (
            <>
              <h3 className="dd-subnav-title">Notebooks</h3>
              <SubRow label="All notebooks" on={page === "notebooks"} onClick={() => onPage("notebooks")} />
              <SubRow label="Docs" on={page === "docs"} onClick={() => onPage("docs")} />
              <SubRow label="Open with Bits RCA" on={page === "bits"} onClick={() => onPage("bits")} />
            </>
          )}
        </aside>
      )}
    </div>
  );
}
