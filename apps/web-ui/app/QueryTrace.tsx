"use client";

import { FormEvent, useMemo, useState } from "react";

import { ApiResult, JsonValue, postJson } from "../lib/api";

const traceQueryTypes = [
  "find-symbol",
  "search-code",
  "impact-analysis",
  "context-pack",
  "related-symbols",
  "trace-dependency"
] as const;

type TraceQueryType = (typeof traceQueryTypes)[number];
type RawTab = "request" | "response" | "trace";

type TraceForm = {
  query: string;
  queryType: TraceQueryType;
  projectId: string;
  branchId: string;
  tokenBudget: number;
  maxDepth: number;
  limit: number;
  minConfidence: number;
  includeTrace: boolean;
};

type TracePayload = {
  query?: string;
  symbol?: string;
  scope: {
    project_id: string;
    branch_id: string;
  };
  include_trace: boolean;
  token_budget: number;
  max_depth: number;
  limit: number;
  min_confidence: number;
};

const initialForm: TraceForm = {
  query: "run",
  queryType: "find-symbol",
  projectId: "default",
  branchId: "main",
  tokenBudget: 1200,
  maxDepth: 2,
  limit: 20,
  minConfidence: 0,
  includeTrace: true
};

export function QueryTrace() {
  const [form, setForm] = useState(initialForm);
  const [lastRequest, setLastRequest] = useState<TracePayload>();
  const [result, setResult] = useState<ApiResult<JsonValue>>();
  const [loading, setLoading] = useState(false);
  const [rawTab, setRawTab] = useState<RawTab>("response");

  const response = result?.ok ? result.data : result?.data;
  const responseObject = objectValue(response);
  const trace = traceValue(response);
  const traceObject = objectValue(trace);
  const traceStages = useMemo(() => buildTimeline(response), [response]);
  const rankingRows = useMemo(() => extractRankingRows(response), [response]);
  const context = useMemo(() => extractContext(response), [response]);
  const savings = useMemo(() => extractSavings(response), [response]);
  const isPartial =
    booleanValue(responseObject?.partial) ||
    stringValue(responseObject?.status) === "not_implemented";

  async function submitTrace(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const validation = validateForm(form);
    if (validation) {
      setResult({ ok: false, error: validation });
      return;
    }

    const payload = buildPayload(form);
    setLastRequest(payload);
    setLoading(true);
    setResult(undefined);
    setResult(await postJson<JsonValue>(`/api/query/${form.queryType}`, payload));
    setLoading(false);
  }

  return (
    <section id="trace" className="band">
      <div className="section-heading">
        <p className="eyebrow">Query Trace</p>
        <h2>Retrieval Explainability</h2>
      </div>

      <form className="trace-form" onSubmit={submitTrace}>
        <label>
          Query Type
          <select
            value={form.queryType}
            onChange={(event) =>
              setForm({ ...form, queryType: event.target.value as TraceQueryType })
            }
          >
            {traceQueryTypes.map((queryType) => (
              <option key={queryType} value={queryType}>
                {queryType}
              </option>
            ))}
          </select>
        </label>
        <label>
          Query Text
          <input
            value={form.query}
            onChange={(event) => setForm({ ...form, query: event.target.value })}
            placeholder="symbol, code text, or dependency target"
          />
        </label>
        <label>
          Project ID
          <input
            value={form.projectId}
            onChange={(event) =>
              setForm({ ...form, projectId: event.target.value })
            }
          />
        </label>
        <label>
          Branch ID
          <input
            value={form.branchId}
            onChange={(event) => setForm({ ...form, branchId: event.target.value })}
          />
        </label>
        <label>
          Token Budget
          <input
            min={1}
            type="number"
            value={form.tokenBudget}
            onChange={(event) =>
              setForm({ ...form, tokenBudget: Number(event.target.value) })
            }
          />
        </label>
        <label>
          Max Depth
          <input
            min={1}
            max={3}
            type="number"
            value={form.maxDepth}
            onChange={(event) =>
              setForm({ ...form, maxDepth: Number(event.target.value) })
            }
          />
        </label>
        <label>
          Limit
          <input
            min={1}
            max={200}
            type="number"
            value={form.limit}
            onChange={(event) =>
              setForm({ ...form, limit: Number(event.target.value) })
            }
          />
        </label>
        <label>
          Min Confidence
          <input
            min={0}
            max={10000}
            type="number"
            value={form.minConfidence}
            onChange={(event) =>
              setForm({ ...form, minConfidence: Number(event.target.value) })
            }
          />
        </label>
        <label className="check-row">
          <input
            checked={form.includeTrace}
            type="checkbox"
            onChange={(event) =>
              setForm({ ...form, includeTrace: event.target.checked })
            }
          />
          Include trace
        </label>
        <button type="submit">{loading ? "Running" : "Run Trace"}</button>
      </form>

      {result && !result.ok && <p className="error">Request failed: {result.error}</p>}
      {isPartial && (
        <p className="note">
          The endpoint marked this response as placeholder or partial; only returned
          fields are shown.
        </p>
      )}

      <div className="trace-grid">
        <TraceTimeline stages={traceStages} />
        <TokenSavingsPanel savings={savings} response={responseObject} />
      </div>

      <RankingPanel rows={rankingRows} />
      <ContextInspector context={context} response={responseObject} />

      <section className="subsection">
        <div className="section-heading">
          <p className="eyebrow">Raw JSON</p>
          <h3>Debug Payloads</h3>
        </div>
        <div className="tab-row">
          {(["request", "response", "trace"] as const).map((tab) => (
            <button
              className={rawTab === tab ? "secondary active" : "secondary"}
              key={tab}
              type="button"
              onClick={() => setRawTab(tab)}
            >
              {tab}
            </button>
          ))}
        </div>
        <pre>
          {JSON.stringify(
            rawTab === "request"
              ? lastRequest ?? buildPayload(form)
              : rawTab === "trace"
                ? trace ?? null
                : response ?? null,
            null,
            2
          )}
        </pre>
      </section>
    </section>
  );
}

function TraceTimeline({ stages }: { stages: TimelineStage[] }) {
  return (
    <section className="trace-panel">
      <h3>Trace Timeline</h3>
      {stages.length === 0 ? (
        <p className="note">No trace stages returned yet.</p>
      ) : (
        <ol className="timeline">
          {stages.map((stage) => (
            <li key={stage.label}>
              <strong>{stage.label}</strong>
              <span>{stage.summary}</span>
              {stage.payload !== undefined && (
                <pre>{JSON.stringify(stage.payload, null, 2)}</pre>
              )}
            </li>
          ))}
        </ol>
      )}
    </section>
  );
}

function RankingPanel({ rows }: { rows: RankingRow[] }) {
  return (
    <section className="subsection">
      <div className="section-heading">
        <p className="eyebrow">Ranking Decisions</p>
        <h3>Score Contributions</h3>
      </div>
      {rows.length === 0 ? (
        <p className="note">No ranking decision rows returned by the endpoint.</p>
      ) : (
        <div className="table-wrap">
          <table>
            <thead>
              <tr>
                <th>Item</th>
                <th>Type</th>
                <th>Base</th>
                <th>Exact</th>
                <th>BM25</th>
                <th>Graph</th>
                <th>Confidence</th>
                <th>Centrality</th>
                <th>Penalties</th>
                <th>Final</th>
                <th>Reason</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((row, index) => (
                <tr key={`${row.itemId}-${index}`}>
                  <td>{row.itemId}</td>
                  <td>{row.itemType}</td>
                  <td>{displayValue(row.baseScore)}</td>
                  <td>{displayValue(row.exactMatch)}</td>
                  <td>{displayValue(row.bm25)}</td>
                  <td>{displayValue(row.graphDistance)}</td>
                  <td>{displayValue(row.edgeConfidence)}</td>
                  <td>{displayValue(row.centrality)}</td>
                  <td>{displayValue(row.penalties)}</td>
                  <td>{displayValue(row.finalScore)}</td>
                  <td>{row.reason ?? "not returned"}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}

function ContextInspector({
  context,
  response
}: {
  context: ContextSummary;
  response?: Record<string, JsonValue>;
}) {
  const expansionHandles = arrayValue(
    response?.expansion_handles ?? objectValue(response?.query_result)?.expansion_handles
  );
  return (
    <section className="subsection">
      <div className="section-heading">
        <p className="eyebrow">Context Pack Inspector</p>
        <h3>Selected and Skipped Items</h3>
      </div>
      <div className="summary-strip">
        <SmallMetric label="Selected" value={String(context.selected.length)} />
        <SmallMetric label="Skipped" value={String(context.skipped.length)} />
        <SmallMetric
          label="Token Estimate"
          value={displayValue(context.tokenEstimate)}
        />
        <SmallMetric
          label="Used Budget"
          value={displayValue(context.usedTokenBudget)}
        />
        <SmallMetric
          label="Total Budget"
          value={displayValue(context.totalTokenBudget)}
        />
      </div>
      <PropertyLine label="Truncation Reason" value={context.truncationReason} />
      <PropertyLine
        label="Expansion Handles"
        value={expansionHandles.length > 0 ? expansionHandles.join(", ") : undefined}
      />
      <div className="context-columns">
        <ContextList title="Selected Items" items={context.selected} />
        <ContextList title="Skipped Items" items={context.skipped} />
      </div>
    </section>
  );
}

function TokenSavingsPanel({
  savings,
  response
}: {
  savings: TokenSavingsSummary;
  response?: Record<string, JsonValue>;
}) {
  return (
    <section className="trace-panel">
      <h3>Token Savings</h3>
      <div className="summary-strip compact">
        <SmallMetric
          label="Raw Tokens"
          value={displayValue(savings.estimatedRawTokens)}
        />
        <SmallMetric label="Returned" value={displayValue(savings.returnedTokens)} />
        <SmallMetric label="Saved" value={displayValue(savings.estimatedSaved)} />
        <SmallMetric label="File Reads" value={displayValue(savings.avoidedFileReads)} />
        <SmallMetric label="Grep Calls" value={displayValue(savings.avoidedGrepCalls)} />
        <SmallMetric label="Ratio" value={displayValue(savings.compressionRatio)} />
      </div>
      {booleanValue(response?.partial) && (
        <p className="note">Savings data is partial or placeholder for this response.</p>
      )}
    </section>
  );
}

function ContextList({
  title,
  items
}: {
  title: string;
  items: ContextItem[];
}) {
  return (
    <div className="context-list">
      <h4>{title}</h4>
      {items.length === 0 ? (
        <p className="note">No items returned.</p>
      ) : (
        items.map((item, index) => <ContextItemView item={item} key={index} />)
      )}
    </div>
  );
}

function ContextItemView({ item }: { item: ContextItem }) {
  const [expanded, setExpanded] = useState(false);
  const snippet = item.snippet ?? "";
  const isLong = snippet.length > 360;
  const displayedSnippet = !expanded && isLong ? `${snippet.slice(0, 360)}...` : snippet;

  return (
    <article className="context-item">
      <strong>{item.id ?? "unknown item"}</strong>
      <span>{item.itemType ?? "unknown type"}</span>
      <PropertyLine label="Reason" value={item.reason} />
      <PropertyLine label="Provenance" value={item.provenance} />
      <PropertyLine label="Tokens" value={item.tokenEstimate} />
      {snippet && <pre>{displayedSnippet}</pre>}
      {isLong && (
        <button className="secondary" type="button" onClick={() => setExpanded(!expanded)}>
          {expanded ? "Collapse Snippet" : "Expand Snippet"}
        </button>
      )}
    </article>
  );
}

function PropertyLine({
  label,
  value
}: {
  label: string;
  value?: string | number | null;
}) {
  return (
    <p className="property-inline">
      <span>{label}</span>
      <strong>{value ?? "not returned"}</strong>
    </p>
  );
}

function SmallMetric({ label, value }: { label: string; value: string }) {
  return (
    <div className="metric small-metric">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

type TimelineStage = {
  label: string;
  summary: string;
  payload?: JsonValue;
};

type RankingRow = {
  itemId: string;
  itemType: string;
  baseScore?: JsonValue;
  exactMatch?: JsonValue;
  bm25?: JsonValue;
  graphDistance?: JsonValue;
  edgeConfidence?: JsonValue;
  centrality?: JsonValue;
  penalties?: JsonValue;
  finalScore?: JsonValue;
  reason?: string;
};

type ContextItem = {
  id?: string;
  itemType?: string;
  snippet?: string;
  reason?: string;
  provenance?: string;
  tokenEstimate?: number;
};

type ContextSummary = {
  selected: ContextItem[];
  skipped: ContextItem[];
  tokenEstimate?: number;
  totalTokenBudget?: number;
  usedTokenBudget?: number;
  truncationReason?: string;
};

type TokenSavingsSummary = {
  estimatedRawTokens?: JsonValue;
  returnedTokens?: JsonValue;
  estimatedSaved?: JsonValue;
  avoidedFileReads?: JsonValue;
  avoidedGrepCalls?: JsonValue;
  compressionRatio?: JsonValue;
};

function buildPayload(form: TraceForm): TracePayload {
  const payload: TracePayload = {
    scope: {
      project_id: form.projectId,
      branch_id: form.branchId
    },
    include_trace: form.includeTrace,
    token_budget: form.tokenBudget,
    max_depth: bounded(form.maxDepth, 1, 3),
    limit: bounded(form.limit, 1, 200),
    min_confidence: bounded(form.minConfidence, 0, 10000)
  };

  if (form.queryType === "find-symbol") {
    payload.symbol = form.query;
  } else {
    payload.query = form.query;
  }

  return payload;
}

function validateForm(form: TraceForm) {
  if (!form.projectId.trim()) {
    return "scope.project_id is required";
  }
  if (!form.branchId.trim()) {
    return "scope.branch_id is required";
  }
  if (!form.query.trim()) {
    return "query text is required";
  }
  if (form.tokenBudget <= 0) {
    return "token_budget must be greater than zero";
  }
  if (form.maxDepth <= 0 || form.maxDepth > 3) {
    return "max_depth must be between 1 and 3";
  }
  if (form.limit <= 0 || form.limit > 200) {
    return "limit must be between 1 and 200";
  }
  if (form.minConfidence < 0 || form.minConfidence > 10000) {
    return "min_confidence must be between 0 and 10000";
  }
  return undefined;
}

function buildTimeline(response: JsonValue | undefined): TimelineStage[] {
  const root = objectValue(response);
  const trace = objectValue(traceValue(response));
  const queryResult = objectValue(root?.query_result);
  const stages: TimelineStage[] = [];

  addStage(stages, "Query Input", trace?.query_input ?? root?.query ?? root?.operation);
  addStage(stages, "Query Intent", trace?.query_intent ?? trace?.intent);
  addStage(stages, "Exact Hits", trace?.exact_hits ?? root?.matches);
  addStage(stages, "FTS Hits", trace?.fts_hits ?? trace?.bm25_hits);
  addStage(stages, "Graph Traversal", trace?.graph_traversal_steps);
  addStage(stages, "Ranking Decisions", trace?.ranking_decisions);
  addStage(stages, "Selected Context", trace?.selected_context_items ?? trace?.selected_items);
  addStage(stages, "Skipped Context", trace?.skipped_context_items ?? trace?.skipped_items);
  addStage(stages, "Truncation", trace?.truncation_metadata ?? trace?.truncation);
  addStage(
    stages,
    "Token Budget",
    trace?.token_budget_used ?? queryResult?.returned_tokens ?? root?.returned_tokens
  );
  addStage(stages, "Token Savings", trace?.token_savings ?? root?.token_savings);
  addStage(stages, "Warnings", trace?.warnings ?? root?.message);

  return stages;
}

function addStage(stages: TimelineStage[], label: string, value: JsonValue | undefined) {
  if (value === undefined || value === null) {
    return;
  }
  const summary =
    typeof value === "string"
      ? value
      : Array.isArray(value)
        ? `${value.length} item(s)`
        : "metadata returned";
  stages.push({ label, summary, payload: typeof value === "string" ? undefined : value });
}

function extractRankingRows(response: JsonValue | undefined): RankingRow[] {
  const trace = objectValue(traceValue(response));
  const rows = arrayValue(
    trace?.ranking_decisions ?? objectValue(response)?.ranking_decisions
  );
  return rows.map((row, index) => {
    const item = objectValue(row);
    return {
      itemId: stringValue(item?.item_id ?? item?.id) ?? `item-${index + 1}`,
      itemType: stringValue(item?.item_type ?? item?.type) ?? "unknown",
      baseScore: item?.base_score,
      exactMatch: item?.exact_match_contribution ?? item?.exact_match,
      bm25: item?.bm25_contribution ?? item?.bm25,
      graphDistance: item?.graph_distance_contribution ?? item?.graph_distance,
      edgeConfidence: item?.edge_confidence_contribution ?? item?.edge_confidence,
      centrality: item?.centrality_contribution ?? item?.centrality,
      penalties: item?.penalties,
      finalScore: item?.final_score ?? item?.score,
      reason: stringValue(item?.reason)
    };
  });
}

function extractContext(response: JsonValue | undefined): ContextSummary {
  const root = objectValue(response);
  const trace = objectValue(traceValue(response));
  const context = objectValue(root?.context_pack ?? trace?.context_pack);

  return {
    selected: extractContextItems(
      context?.selected_items ?? trace?.selected_context_items ?? root?.selected_items
    ),
    skipped: extractContextItems(
      context?.skipped_items ?? trace?.skipped_context_items ?? root?.skipped_items
    ),
    tokenEstimate: numberValue(context?.token_estimate ?? trace?.token_estimate),
    totalTokenBudget: numberValue(
      context?.total_token_budget ?? trace?.total_token_budget
    ),
    usedTokenBudget: numberValue(context?.used_token_budget ?? trace?.used_token_budget),
    truncationReason: stringValue(
      context?.truncation_reason ?? trace?.truncation_reason
    )
  };
}

function extractContextItems(value: JsonValue | undefined): ContextItem[] {
  return arrayValue(value).map((entry, index) => {
    const item = objectValue(entry);
    return {
      id: stringValue(item?.id ?? item?.item_id) ?? `item-${index + 1}`,
      itemType: stringValue(item?.item_type ?? item?.type),
      snippet: stringValue(item?.snippet ?? item?.content),
      reason: stringValue(item?.reason ?? item?.skip_reason),
      provenance: stringValue(item?.provenance ?? item?.source_provenance),
      tokenEstimate: numberValue(item?.token_estimate ?? item?.tokens)
    };
  });
}

function extractSavings(response: JsonValue | undefined): TokenSavingsSummary {
  const root = objectValue(response);
  const trace = objectValue(traceValue(response));
  const savings = objectValue(root?.token_savings ?? trace?.token_savings);
  const queryResult = objectValue(root?.query_result);

  return {
    estimatedRawTokens: savings?.estimated_raw_tokens,
    returnedTokens:
      savings?.returned_tokens ?? queryResult?.returned_tokens ?? root?.returned_tokens,
    estimatedSaved: savings?.estimated_tokens_saved,
    avoidedFileReads: savings?.avoided_file_reads,
    avoidedGrepCalls: savings?.avoided_grep_calls ?? savings?.avoided_search_calls,
    compressionRatio: savings?.compression_ratio
  };
}

function traceValue(response: JsonValue | undefined): JsonValue | undefined {
  const root = objectValue(response);
  return root?.trace_payload ?? root?.query_trace ?? root?.trace;
}

function bounded(value: number, min: number, max: number) {
  return Math.min(Math.max(value, min), max);
}

function objectValue(value: JsonValue | undefined) {
  if (value && typeof value === "object" && !Array.isArray(value)) {
    return value;
  }
  return undefined;
}

function arrayValue(value: JsonValue | undefined) {
  return Array.isArray(value) ? value : [];
}

function stringValue(value: JsonValue | undefined) {
  return typeof value === "string" ? value : undefined;
}

function numberValue(value: JsonValue | undefined) {
  return typeof value === "number" ? value : undefined;
}

function booleanValue(value: JsonValue | undefined) {
  return typeof value === "boolean" ? value : undefined;
}

function displayValue(value: JsonValue | undefined) {
  if (value === undefined || value === null) {
    return "not returned";
  }
  if (typeof value === "object") {
    return JSON.stringify(value);
  }
  return String(value);
}
