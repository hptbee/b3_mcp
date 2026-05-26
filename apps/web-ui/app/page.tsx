"use client";

import { FormEvent, useEffect, useMemo, useState } from "react";

import { GraphExplorer } from "./GraphExplorer";
import { QueryTrace } from "./QueryTrace";
import {
  API_BASE_URL,
  ApiResult,
  getJson,
  HealthResponse,
  IndexStatusResponse,
  IndexSummary,
  JsonValue,
  postJson,
  ProjectResponse,
  QueryPayload,
  SavingsSummary,
  StatusResponse
} from "../lib/api";

const queryOperations = [
  "find-symbol",
  "search-code",
  "impact-analysis",
  "context-pack"
] as const;

type QueryOperation = (typeof queryOperations)[number];

type DashboardState = {
  health?: ApiResult<HealthResponse>;
  status?: ApiResult<StatusResponse>;
  project?: ApiResult<ProjectResponse>;
  capabilities?: ApiResult<JsonValue>;
  savings?: ApiResult<SavingsSummary>;
  diagnostics?: ApiResult<JsonValue>;
  config?: ApiResult<JsonValue>;
  indexStatus?: ApiResult<IndexStatusResponse>;
};

type EventMessage = {
  type: string;
  data: string;
  timestamp: string;
};

export default function Home() {
  const [state, setState] = useState<DashboardState>({});
  const [loading, setLoading] = useState(true);
  const [queryOperation, setQueryOperation] =
    useState<QueryOperation>("find-symbol");
  const [queryText, setQueryText] = useState("run");
  const [projectId, setProjectId] = useState("default");
  const [branchId, setBranchId] = useState("main");
  const [queryResult, setQueryResult] = useState<ApiResult<JsonValue>>();
  const [configInput, setConfigInput] = useState("{\n  \"offline\": true\n}");
  const [configValidation, setConfigValidation] =
    useState<ApiResult<JsonValue>>();
  const [events, setEvents] = useState<EventMessage[]>([]);
  const [eventStatus, setEventStatus] = useState("disconnected");
  const [indexAction, setIndexAction] = useState<"idle" | "index" | "reindex">(
    "idle"
  );
  const [indexResult, setIndexResult] = useState<ApiResult<IndexSummary>>();

  useEffect(() => {
    void refresh();
  }, []);

  useEffect(() => {
    const source = new EventSource(`${API_BASE_URL}/api/events`);
    setEventStatus("connecting");

    source.onopen = () => setEventStatus("connected");
    source.onerror = () => setEventStatus("disconnected");
    source.onmessage = (message) => {
      setEvents((current) => [
        {
          type: "message",
          data: message.data,
          timestamp: new Date().toISOString()
        },
        ...current
      ]);
    };
    source.addEventListener("server_started", (message) => {
      setEvents((current) => [
        {
          type: "server_started",
          data: message.data,
          timestamp: new Date().toISOString()
        },
        ...current
      ]);
    });
    for (const eventName of [
      "indexing_started",
      "file_indexed",
      "file_skipped",
      "indexing_completed",
      "indexing_failed",
      "parse_failed"
    ]) {
      source.addEventListener(eventName, (message) => {
        setEvents((current) => [
          {
            type: eventName,
            data: message.data,
            timestamp: new Date().toISOString()
          },
          ...current
        ]);
      });
    }

    return () => source.close();
  }, []);

  const status = state.status?.ok ? state.status.data : undefined;
  const project = state.project?.ok ? state.project.data : undefined;
  const health = state.health?.ok ? state.health.data : undefined;
  const savings = state.savings?.ok ? state.savings.data : undefined;
  const indexStatus =
    state.indexStatus?.ok ? state.indexStatus.data : undefined;

  const capabilitySummary = useMemo(() => {
    if (!state.capabilities?.ok) {
      return "Unavailable";
    }
    return JSON.stringify(state.capabilities.data, null, 2);
  }, [state.capabilities]);

  async function refresh() {
    setLoading(true);
    const [
      healthResponse,
      statusResponse,
      projectResponse,
      capabilitiesResponse,
      savingsResponse,
      diagnosticsResponse,
      configResponse,
      indexStatusResponse
    ] = await Promise.all([
      getJson<HealthResponse>("/health"),
      getJson<StatusResponse>("/api/status"),
      getJson<ProjectResponse>("/api/project"),
      getJson<JsonValue>("/api/capabilities"),
      getJson<SavingsSummary>("/api/savings/summary"),
      getJson<JsonValue>("/api/diagnostics"),
      getJson<JsonValue>("/api/config"),
      getJson<IndexStatusResponse>("/api/index/status")
    ]);

    setState({
      health: healthResponse,
      status: statusResponse,
      project: projectResponse,
      capabilities: capabilitiesResponse,
      savings: savingsResponse,
      diagnostics: diagnosticsResponse,
      config: configResponse,
      indexStatus: indexStatusResponse
    });
    setLoading(false);
  }

  async function runIndex(reindex: boolean) {
    setIndexAction(reindex ? "reindex" : "index");
    setIndexResult(undefined);
    const result = await postJson<IndexSummary>(
      reindex ? "/api/index/reindex" : "/api/index/run",
      {}
    );
    setIndexResult(result);
    await refresh();
    setIndexAction("idle");
  }

  async function submitQuery(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const payload: QueryPayload = {
      scope: {
        project_id: projectId,
        branch_id: branchId
      },
      include_trace: true,
      limit: 20,
      token_budget: 4000
    };

    if (queryOperation === "find-symbol") {
      payload.symbol = queryText;
    } else {
      payload.query = queryText;
    }

    setQueryResult(undefined);
    setQueryResult(
      await postJson<JsonValue>(`/api/query/${queryOperation}`, payload)
    );
  }

  async function validateConfig() {
    let parsed: unknown = configInput;
    try {
      parsed = JSON.parse(configInput);
    } catch {
      parsed = { raw: configInput };
    }

    setConfigValidation(await postJson<JsonValue>("/api/config/validate", parsed));
  }

  return (
    <main className="shell bg-zinc-950 text-zinc-100">
      <aside className="sidebar">
        <div>
          <p className="eyebrow">B3 Control</p>
          <h1>Local MCP Intelligence</h1>
        </div>
        <div className="button-row">
          <StatusChip label={health?.status ?? "unknown"} tone={statusTone(health?.status)} />
          <StatusChip
            label={eventStatus}
            tone={eventStatus === "connected" ? "green" : eventStatus === "connecting" ? "blue" : "amber"}
          />
        </div>
        <nav aria-label="Sections">
          <a href="#dashboard">Dashboard</a>
          <a href="#project">Project Status</a>
          <a href="#query">Query Playground</a>
          <a href="#trace">Query Trace</a>
          <a href="#graph">Graph Explorer</a>
          <a href="#savings">Token Savings</a>
          <a href="#diagnostics">Diagnostics</a>
          <a href="#config">Config Viewer</a>
          <a href="#capabilities">Capabilities</a>
          <a href="#events">Logs / Events</a>
        </nav>
        <button type="button" onClick={refresh}>
          {loading ? "Refreshing" : "Refresh"}
        </button>
      </aside>

      <section className="content">
        <section id="dashboard" className="band">
          <div className="section-heading">
            <p className="eyebrow">Dashboard</p>
            <h2>Server Overview</h2>
          </div>
          <div className="metric-grid">
            <Metric label="Health" value={health?.status ?? "unknown"} />
            <Metric
              label="Offline Mode"
              value={String(health?.offline_mode ?? status?.offline_mode ?? false)}
            />
            <Metric
              label="Telemetry"
              value={health?.telemetry_enabled ? "enabled" : "disabled"}
            />
            <Metric label="API Base URL" value={API_BASE_URL} />
          </div>
          <div className="button-row">
            <StatusChip label={state.health?.ok ? "api online" : "api offline"} tone={state.health?.ok ? "green" : "rose"} />
            <StatusChip label={health?.offline_mode ?? status?.offline_mode ? "offline mode" : "online capable"} tone={health?.offline_mode ?? status?.offline_mode ? "green" : "amber"} />
            <StatusChip label={health?.telemetry_enabled ? "telemetry enabled" : "telemetry disabled"} tone={health?.telemetry_enabled ? "rose" : "green"} />
          </div>
          <ErrorLine result={state.health} />
        </section>

        <section id="project" className="band">
          <div className="section-heading">
            <p className="eyebrow">Project Status</p>
            <h2>Local Index Snapshot</h2>
          </div>
          <dl className="details">
            <Field label="Project Path" value={project?.path ?? status?.project_path} />
            <Field
              label="Database Path"
              value={project?.database_path ?? status?.database_path}
            />
            <Field label="Current Branch" value={status?.current_branch ?? "not indexed"} />
            <Field
              label="Indexed Files"
              value={String(project?.indexed_file_count ?? status?.indexed_file_count ?? 0)}
            />
            <Field
              label="Symbols"
              value={String(project?.symbol_count ?? status?.symbol_count ?? 0)}
            />
            <Field
              label="Edges"
              value={String(project?.edge_count ?? status?.edge_count ?? 0)}
            />
          </dl>
          <div className="button-row">
            <button
              type="button"
              onClick={() => void runIndex(false)}
              disabled={indexAction !== "idle"}
            >
              {indexAction === "index" ? "Indexing" : "Run Index"}
            </button>
            <button
              type="button"
              onClick={() => void runIndex(true)}
              disabled={indexAction !== "idle"}
            >
              {indexAction === "reindex" ? "Reindexing" : "Reindex Project"}
            </button>
          </div>
          <div className="metric-grid">
            <Metric label="Index Status" value={indexStatus?.status ?? "idle"} />
            <Metric
              label="Files Discovered"
              value={String(indexStatus?.files_discovered ?? 0)}
            />
            <Metric
              label="Files Indexed"
              value={String(indexStatus?.files_indexed ?? 0)}
            />
            <Metric
              label="Files Skipped"
              value={String(indexStatus?.files_skipped ?? 0)}
            />
            <Metric
              label="Parse Failures"
              value={String(indexStatus?.parse_failures ?? 0)}
            />
            <Metric
              label="Last Duration"
              value={
                indexStatus?.duration_ms == null
                  ? "0 ms"
                  : `${indexStatus.duration_ms} ms`
              }
            />
          </div>
          {indexStatus?.last_error && (
            <p className="error">{indexStatus.last_error}</p>
          )}
          <JsonPanel title="Last Index Summary" result={indexResult} />
          <p className="note">
            Counts reflect the local control server storage view. Reindex uses
            the current safe incremental behavior and skips unchanged files.
          </p>
        </section>

        <section id="query" className="band">
          <div className="section-heading">
            <p className="eyebrow">Query Playground</p>
            <h2>Control API Probe</h2>
          </div>
          <form className="query-form" onSubmit={submitQuery}>
            <label>
              Operation
              <select
                value={queryOperation}
                onChange={(event) =>
                  setQueryOperation(event.target.value as QueryOperation)
                }
              >
                {queryOperations.map((operation) => (
                  <option key={operation} value={operation}>
                    {operation}
                  </option>
                ))}
              </select>
            </label>
            <label>
              Query / Symbol
              <input
                value={queryText}
                onChange={(event) => setQueryText(event.target.value)}
                placeholder="symbol or search text"
              />
            </label>
            <label>
              Project ID
              <input
                value={projectId}
                onChange={(event) => setProjectId(event.target.value)}
              />
            </label>
            <label>
              Branch ID
              <input
                value={branchId}
                onChange={(event) => setBranchId(event.target.value)}
              />
            </label>
            <button type="submit">Run Query</button>
          </form>
          <JsonPanel title="Raw Response" result={queryResult} />
        </section>

        <QueryTrace />

        <GraphExplorer />

        <section id="savings" className="band">
          <div className="section-heading">
            <p className="eyebrow">Token Savings</p>
            <h2>Ledger Summary</h2>
          </div>
          <div className="metric-grid">
            <Metric
              label="Estimated Tokens Saved"
              value={String(savings?.estimated_tokens_saved ?? 0)}
            />
            <Metric
              label="Avoided File Reads"
              value={String(savings?.avoided_file_reads ?? 0)}
            />
            <Metric
              label="Avoided Grep Calls"
              value={String(savings?.avoided_search_calls ?? 0)}
            />
            <Metric
              label="Returned Tokens"
              value={String(savings?.returned_tokens ?? 0)}
            />
          </div>
          <p className="note">
            {savings?.partial
              ? "Partial placeholder data from the control API."
              : "Ledger values are reported by local storage when available."}
          </p>
        </section>

        <section id="diagnostics" className="band">
          <div className="section-heading">
            <p className="eyebrow">Diagnostics</p>
            <h2>Local Runtime Signals</h2>
          </div>
          <JsonPanel title="Diagnostics JSON" result={state.diagnostics} />
        </section>

        <section id="config" className="band">
          <div className="section-heading">
            <p className="eyebrow">Config Viewer</p>
            <h2>Read and Validate</h2>
          </div>
          <JsonPanel title="Current Config" result={state.config} />
          <textarea
            value={configInput}
            onChange={(event) => setConfigInput(event.target.value)}
            rows={8}
            aria-label="Config validation input"
          />
          <button type="button" onClick={validateConfig}>
            Validate Config
          </button>
          <JsonPanel title="Validation Result" result={configValidation} />
        </section>

        <section id="capabilities" className="band">
          <div className="section-heading">
            <p className="eyebrow">Capabilities</p>
            <h2>Available Local APIs</h2>
          </div>
          <pre>{capabilitySummary}</pre>
        </section>

        <section id="events" className="band">
          <div className="section-heading">
            <p className="eyebrow">Logs / Events</p>
            <h2>SSE Feed</h2>
          </div>
          <StatusChip
            label={`SSE ${eventStatus}`}
            tone={eventStatus === "connected" ? "green" : eventStatus === "connecting" ? "blue" : "amber"}
          />
          {events.length === 0 ? (
            <p className="note">Waiting for control server events.</p>
          ) : (
            <ul className="events">
              {events.map((event, index) => (
                <li key={`${event.timestamp}-${index}`}>
                  <strong>{event.type}</strong>
                  <span>{event.timestamp}</span>
                  <code>{event.data}</code>
                </li>
              ))}
            </ul>
          )}
        </section>
      </section>
    </main>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="metric">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

type ChipTone = "green" | "blue" | "amber" | "rose";

function StatusChip({ label, tone }: { label: string; tone: ChipTone }) {
  return <span className={`status-chip ${tone}`}>{label}</span>;
}

function statusTone(status?: string): ChipTone {
  const normalized = status?.toLowerCase() ?? "";
  if (["ok", "ready", "healthy", "running"].some((value) => normalized.includes(value))) {
    return "green";
  }
  if (["loading", "indexing", "connecting"].some((value) => normalized.includes(value))) {
    return "blue";
  }
  if (["error", "failed", "offline"].some((value) => normalized.includes(value))) {
    return "rose";
  }
  return "amber";
}

function Field({
  label,
  value
}: {
  label: string;
  value?: string | number | null;
}) {
  return (
    <div>
      <dt>{label}</dt>
      <dd>{value ?? "unavailable"}</dd>
    </div>
  );
}

function ErrorLine<T>({ result }: { result?: ApiResult<T> }) {
  if (!result || result.ok) {
    return null;
  }

  return <p className="error">Request failed: {result.error}</p>;
}

function JsonPanel<T>({
  title,
  result
}: {
  title: string;
  result?: ApiResult<T>;
}) {
  if (!result) {
    return (
      <div className="json-panel">
        <h3>{title}</h3>
        <pre>Not loaded yet.</pre>
      </div>
    );
  }

  return (
    <div className="json-panel">
      <h3>{title}</h3>
      {!result.ok && <p className="error">{result.error}</p>}
      <pre>{JSON.stringify(result.ok ? result.data : result.data, null, 2)}</pre>
    </div>
  );
}
