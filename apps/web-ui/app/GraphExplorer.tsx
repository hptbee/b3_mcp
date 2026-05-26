"use client";

import {
  Background,
  Controls,
  Edge,
  MarkerType,
  MiniMap,
  Node,
  ReactFlow,
  useEdgesState,
  useNodesState
} from "@xyflow/react";
import { FormEvent, useMemo, useState } from "react";

import { ApiResult, getJson, JsonValue, postJson } from "../lib/api";

type Direction = "inbound" | "outbound" | "both";
type InspectorSelection =
  | { kind: "node"; value: GraphNodeRecord }
  | { kind: "edge"; value: GraphEdgeRecord };

type GraphControlsState = {
  projectId: string;
  branchId: string;
  seedId: string;
  edgeTypes: string;
  direction: Direction;
  maxDepth: number;
  minConfidence: number;
  limit: number;
};

type PathState = {
  sourceId: string;
  targetId: string;
  maxDepth: number;
  edgeTypes: string;
};

type GraphNodeRecord = Record<string, unknown> & {
  id: string;
  label: string;
  name: string;
  kind?: string;
  file_path?: string;
  symbol_id?: string;
  language?: string;
  visibility?: string;
  centrality?: number;
  branch_id?: string;
  provenance?: string;
  raw: JsonValue;
};

type GraphEdgeRecord = Record<string, unknown> & {
  id: string;
  edge_type: string;
  from_node_id: string;
  to_node_id: string;
  confidence?: number;
  provenance?: string;
  branch_id?: string;
  raw: JsonValue;
};

type GraphPayload = {
  scope: {
    project_id: string;
    branch_id: string;
  };
  node_id?: string;
  source_node_id?: string;
  target_node_id?: string;
  from_node_id?: string;
  to_node_id?: string;
  direction?: Direction;
  depth?: number;
  max_depth?: number;
  min_confidence?: number;
  limit: number;
  edge_types?: string[];
};

type ExplorerNode = Node<GraphNodeRecord>;
type ExplorerEdge = Edge<GraphEdgeRecord>;

const initialControls: GraphControlsState = {
  projectId: "default",
  branchId: "main",
  seedId: "",
  edgeTypes: "",
  direction: "both",
  maxDepth: 1,
  minConfidence: 0,
  limit: 50
};

const initialPath: PathState = {
  sourceId: "",
  targetId: "",
  maxDepth: 3,
  edgeTypes: ""
};

export function GraphExplorer() {
  const [controls, setControls] = useState(initialControls);
  const [pathState, setPathState] = useState(initialPath);
  const [summary, setSummary] = useState<ApiResult<JsonValue>>();
  const [neighbors, setNeighbors] = useState<ApiResult<JsonValue>>();
  const [pathResult, setPathResult] = useState<ApiResult<JsonValue>>();
  const [cycleResult, setCycleResult] = useState<ApiResult<JsonValue>>();
  const [centralityResult, setCentralityResult] = useState<ApiResult<JsonValue>>();
  const [loading, setLoading] = useState(false);
  const [selection, setSelection] = useState<InspectorSelection>();
  const [nodes, setNodes, onNodesChange] = useNodesState<ExplorerNode>([]);
  const [edges, setEdges, onEdgesChange] = useEdgesState<ExplorerEdge>([]);

  const placeholderMessage = useMemo(
    () =>
      firstMessage(neighbors) ??
      firstMessage(pathResult) ??
      "Graph APIs may return placeholder data until backend graph traversal lands.",
    [neighbors, pathResult]
  );

  async function loadSummary() {
    setSummary(await getJson<JsonValue>("/api/graph/summary"));
  }

  async function submitNeighbors(event?: FormEvent<HTMLFormElement>) {
    event?.preventDefault();
    setLoading(true);
    const result = await postJson<JsonValue>("/api/graph/neighbors", {
      ...basePayload(controls),
      node_id: controls.seedId || undefined,
      direction: controls.direction,
      depth: boundedDepth(controls.maxDepth)
    });
    setNeighbors(result);
    applyGraphResult(result);
    setLoading(false);
  }

  async function expandSelectedNode() {
    if (!selection || selection.kind !== "node") {
      return;
    }
    setControls((current) => ({ ...current, seedId: selection.value.id }));
    setLoading(true);
    const result = await postJson<JsonValue>("/api/graph/neighbors", {
      ...basePayload({ ...controls, seedId: selection.value.id }),
      node_id: selection.value.id,
      direction: controls.direction,
      depth: boundedDepth(controls.maxDepth)
    });
    setNeighbors(result);
    applyGraphResult(result);
    setLoading(false);
  }

  async function submitPath(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const result = await postJson<JsonValue>("/api/graph/path", {
      ...basePayload({
        ...controls,
        maxDepth: pathState.maxDepth,
        edgeTypes: pathState.edgeTypes
      }),
      source_node_id: pathState.sourceId,
      target_node_id: pathState.targetId,
      from_node_id: pathState.sourceId,
      to_node_id: pathState.targetId,
      max_depth: boundedDepth(pathState.maxDepth)
    });
    setPathResult(result);
    applyGraphResult(result);
  }

  async function detectCycles() {
    setCycleResult(
      await postJson<JsonValue>("/api/graph/cycles", {
        ...basePayload(controls),
        max_depth: boundedDepth(controls.maxDepth)
      })
    );
  }

  async function loadCentrality() {
    setCentralityResult(
      await postJson<JsonValue>("/api/graph/centrality", basePayload(controls))
    );
  }

  function applyGraphResult(result: ApiResult<JsonValue>) {
    if (!result.ok) {
      setNodes([]);
      setEdges([]);
      return;
    }

    const graphNodes = extractNodes(result.data);
    const graphEdges = extractEdges(result.data);
    setNodes(layoutNodes(graphNodes));
    setEdges(layoutEdges(graphEdges));
  }

  return (
    <section id="graph" className="band">
      <div className="section-heading">
        <p className="eyebrow">Graph Explorer</p>
        <h2>Local Code Relationship Browser</h2>
      </div>

      <div className="graph-toolbar">
        <button type="button" onClick={loadSummary}>
          Load Summary
        </button>
        <span>{placeholderMessage}</span>
      </div>

      <form className="graph-controls" onSubmit={submitNeighbors}>
        <label>
          Project ID
          <input
            value={controls.projectId}
            onChange={(event) =>
              setControls({ ...controls, projectId: event.target.value })
            }
          />
        </label>
        <label>
          Branch ID
          <input
            value={controls.branchId}
            onChange={(event) =>
              setControls({ ...controls, branchId: event.target.value })
            }
          />
        </label>
        <label>
          Seed Node or Symbol ID
          <input
            value={controls.seedId}
            onChange={(event) =>
              setControls({ ...controls, seedId: event.target.value })
            }
            placeholder="node id"
          />
        </label>
        <label>
          Edge Types
          <input
            value={controls.edgeTypes}
            onChange={(event) =>
              setControls({ ...controls, edgeTypes: event.target.value })
            }
            placeholder="calls,imports"
          />
        </label>
        <label>
          Direction
          <select
            value={controls.direction}
            onChange={(event) =>
              setControls({
                ...controls,
                direction: event.target.value as Direction
              })
            }
          >
            <option value="both">both</option>
            <option value="inbound">inbound</option>
            <option value="outbound">outbound</option>
          </select>
        </label>
        <label>
          Max Depth
          <input
            min={1}
            max={3}
            type="number"
            value={controls.maxDepth}
            onChange={(event) =>
              setControls({
                ...controls,
                maxDepth: Number(event.target.value)
              })
            }
          />
        </label>
        <label>
          Min Confidence
          <input
            min={0}
            max={10000}
            type="number"
            value={controls.minConfidence}
            onChange={(event) =>
              setControls({
                ...controls,
                minConfidence: Number(event.target.value)
              })
            }
          />
        </label>
        <label>
          Limit
          <input
            min={1}
            max={200}
            type="number"
            value={controls.limit}
            onChange={(event) =>
              setControls({ ...controls, limit: Number(event.target.value) })
            }
          />
        </label>
        <button type="submit">{loading ? "Loading" : "Load Neighbors"}</button>
      </form>

      <div className="graph-workspace">
        <div className="graph-canvas" aria-label="Code relationship graph">
          {nodes.length === 0 ? (
            <div className="empty-state">
              <strong>No graph data loaded.</strong>
              <span>Load neighbors or a dependency path to render nodes and edges.</span>
            </div>
          ) : (
            <ReactFlow
              nodes={nodes}
              edges={edges}
              fitView
              onNodesChange={onNodesChange}
              onEdgesChange={onEdgesChange}
              onNodeClick={(_, node) =>
                setSelection({ kind: "node", value: node.data })
              }
              onEdgeClick={(_, edge) => {
                if (edge.data) {
                  setSelection({ kind: "edge", value: edge.data });
                }
              }}
            >
              <Background />
              <Controls />
              <MiniMap pannable zoomable />
            </ReactFlow>
          )}
        </div>
        <Inspector selection={selection} onExpand={expandSelectedNode} />
      </div>

      <div className="graph-panels">
        <JsonPanel title="Graph Summary" result={summary} />
        <JsonPanel title="Neighbors Response" result={neighbors} />
      </div>

      <section className="subsection">
        <div className="section-heading">
          <p className="eyebrow">Dependency Path</p>
          <h3>Bounded Path Query</h3>
        </div>
        <form className="path-form" onSubmit={submitPath}>
          <label>
            Source Node ID
            <input
              value={pathState.sourceId}
              onChange={(event) =>
                setPathState({ ...pathState, sourceId: event.target.value })
              }
            />
          </label>
          <label>
            Target Node ID
            <input
              value={pathState.targetId}
              onChange={(event) =>
                setPathState({ ...pathState, targetId: event.target.value })
              }
            />
          </label>
          <label>
            Max Depth
            <input
              min={1}
              max={3}
              type="number"
              value={pathState.maxDepth}
              onChange={(event) =>
                setPathState({
                  ...pathState,
                  maxDepth: Number(event.target.value)
                })
              }
            />
          </label>
          <label>
            Edge Filters
            <input
              value={pathState.edgeTypes}
              onChange={(event) =>
                setPathState({ ...pathState, edgeTypes: event.target.value })
              }
              placeholder="calls,depends_on"
            />
          </label>
          <button type="submit">Find Path</button>
        </form>
        <PathSummary result={pathResult} />
        <JsonPanel title="Path Response" result={pathResult} />
      </section>

      <section className="subsection">
        <div className="section-heading">
          <p className="eyebrow">Cycles</p>
          <h3>Bounded Cycle Detection</h3>
        </div>
        <button type="button" onClick={detectCycles}>
          Detect Cycles
        </button>
        <CycleSummary result={cycleResult} />
        <JsonPanel title="Cycle Response" result={cycleResult} />
      </section>

      <section className="subsection">
        <div className="section-heading">
          <p className="eyebrow">Centrality</p>
          <h3>Important Nodes</h3>
        </div>
        <button type="button" onClick={loadCentrality}>
          Load Centrality
        </button>
        <CentralityTable result={centralityResult} />
        <JsonPanel title="Centrality Response" result={centralityResult} />
      </section>
    </section>
  );
}

function Inspector({
  selection,
  onExpand
}: {
  selection?: InspectorSelection;
  onExpand: () => void;
}) {
  if (!selection) {
    return (
      <aside className="inspector">
        <h3>Property Inspector</h3>
        <p className="note">Select a node or edge to inspect graph metadata.</p>
      </aside>
    );
  }

  if (selection.kind === "edge") {
    const edge = selection.value;
    return (
      <aside className="inspector">
        <h3>Edge</h3>
        <Property label="Edge ID" value={edge.id} />
        <Property label="Edge Type" value={edge.edge_type} />
        <Property label="From Node" value={edge.from_node_id} />
        <Property label="To Node" value={edge.to_node_id} />
        <Property label="Confidence" value={edge.confidence} />
        <Property label="Provenance" value={edge.provenance} />
        <Property label="Branch ID" value={edge.branch_id} />
      </aside>
    );
  }

  const node = selection.value;
  return (
    <aside className="inspector">
      <h3>Node</h3>
      <Property label="Node ID" value={node.id} />
      <Property label="Name" value={node.name} />
      <Property label="Kind" value={node.kind} />
      <Property label="File Path" value={node.file_path} />
      <Property label="Symbol ID" value={node.symbol_id} />
      <Property label="Language" value={node.language} />
      <Property label="Visibility" value={node.visibility} />
      <Property label="Centrality" value={node.centrality} />
      <Property label="Branch ID" value={node.branch_id} />
      <Property label="Provenance" value={node.provenance} />
      <button type="button" onClick={onExpand}>
        Expand Neighbors
      </button>
    </aside>
  );
}

function Property({
  label,
  value
}: {
  label: string;
  value?: string | number | null;
}) {
  return (
    <div className="property-row">
      <span>{label}</span>
      <strong>{value ?? "not available"}</strong>
    </div>
  );
}

function PathSummary({ result }: { result?: ApiResult<JsonValue> }) {
  if (!result?.ok) {
    return <p className="note">No path result loaded.</p>;
  }

  const data = objectValue(result.data);
  const nodes = arrayValue(data?.nodes).length;
  const edges = arrayValue(data?.edges).length;
  const found = booleanValue(data?.path_found) ?? nodes > 0;

  return (
    <div className="summary-strip">
      <Metric label="Path Found" value={found ? "yes" : "not returned"} />
      <Metric label="Ordered Nodes" value={String(nodes)} />
      <Metric label="Ordered Edges" value={String(edges)} />
      <Metric label="Path Length" value={String(Math.max(edges, 0))} />
      <Metric
        label="Confidence"
        value={stringValue(data?.confidence_summary) ?? "not returned"}
      />
    </div>
  );
}

function CycleSummary({ result }: { result?: ApiResult<JsonValue> }) {
  if (!result?.ok) {
    return <p className="note">No cycle result loaded.</p>;
  }

  const data = objectValue(result.data);
  const cycles = arrayValue(data?.cycles);
  const groups = arrayValue(data?.scc_groups);

  return (
    <div className="summary-strip">
      <Metric label="Cycle Count" value={String(numberValue(data?.cycle_count) ?? cycles.length)} />
      <Metric label="SCC Groups" value={String(groups.length)} />
      <Metric
        label="Bounded Warning"
        value={stringValue(data?.bounded_warning) ?? "none returned"}
      />
    </div>
  );
}

function CentralityTable({ result }: { result?: ApiResult<JsonValue> }) {
  if (!result?.ok) {
    return <p className="note">No centrality result loaded.</p>;
  }

  const data = objectValue(result.data);
  const rows = arrayValue(data?.nodes ?? data?.top_nodes);

  if (rows.length === 0) {
    return <p className="note">No centrality rows returned by the control API.</p>;
  }

  return (
    <div className="table-wrap">
      <table>
        <thead>
          <tr>
            <th>Node</th>
            <th>PageRank</th>
            <th>In</th>
            <th>Out</th>
            <th>Fan In</th>
            <th>Fan Out</th>
            <th>Degree</th>
            <th>Component</th>
            <th>Cycle</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((row, index) => {
            const object = objectValue(row);
            return (
              <tr key={`${stringValue(object?.id) ?? "row"}-${index}`}>
                <td>{stringValue(object?.id ?? object?.node_id) ?? "unknown"}</td>
                <td>{displayValue(object?.pagerank)}</td>
                <td>{displayValue(object?.in_degree)}</td>
                <td>{displayValue(object?.out_degree)}</td>
                <td>{displayValue(object?.fan_in)}</td>
                <td>{displayValue(object?.fan_out)}</td>
                <td>{displayValue(object?.degree_centrality)}</td>
                <td>{displayValue(object?.component_size)}</td>
                <td>{displayValue(object?.cycle_membership)}</td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
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

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="metric small-metric">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

function basePayload(controls: GraphControlsState): GraphPayload {
  return {
    scope: {
      project_id: controls.projectId,
      branch_id: controls.branchId
    },
    limit: boundedLimit(controls.limit),
    min_confidence: Math.max(0, controls.minConfidence),
    edge_types: splitCsv(controls.edgeTypes)
  };
}

function boundedLimit(value: number) {
  return Math.min(Math.max(value || 50, 1), 200);
}

function boundedDepth(value: number) {
  return Math.min(Math.max(value || 1, 1), 3);
}

function splitCsv(value: string) {
  const parts = value
    .split(",")
    .map((part) => part.trim())
    .filter(Boolean);
  return parts.length > 0 ? parts : undefined;
}

function extractNodes(data: JsonValue): GraphNodeRecord[] {
  const root = objectValue(data);
  return arrayValue(root?.nodes)
    .map((value) => toNodeRecord(value))
    .filter((value): value is GraphNodeRecord => Boolean(value));
}

function extractEdges(data: JsonValue): GraphEdgeRecord[] {
  const root = objectValue(data);
  return arrayValue(root?.edges)
    .map((value) => toEdgeRecord(value))
    .filter((value): value is GraphEdgeRecord => Boolean(value));
}

function toNodeRecord(value: JsonValue): GraphNodeRecord | undefined {
  const object = objectValue(value);
  const id = stringValue(object?.id ?? object?.node_id);
  if (!object || !id) {
    return undefined;
  }

  return {
    id,
    label: stringValue(object.name ?? object.label) ?? id,
    name: stringValue(object.name ?? object.label) ?? id,
    kind: stringValue(object.kind ?? object.node_kind),
    file_path: stringValue(object.file_path ?? object.path),
    symbol_id: stringValue(object.symbol_id),
    language: stringValue(object.language),
    visibility: stringValue(object.visibility),
    centrality: numberValue(object.centrality ?? object.pagerank),
    branch_id: stringValue(object.branch_id),
    provenance: stringValue(object.provenance),
    raw: value
  };
}

function toEdgeRecord(value: JsonValue): GraphEdgeRecord | undefined {
  const object = objectValue(value);
  const id = stringValue(object?.id ?? object?.edge_id);
  const from = stringValue(object?.from_node_id ?? object?.source);
  const to = stringValue(object?.to_node_id ?? object?.target);
  if (!object || !id || !from || !to) {
    return undefined;
  }

  return {
    id,
    edge_type: stringValue(object.edge_type ?? object.type ?? object.label) ?? "edge",
    from_node_id: from,
    to_node_id: to,
    confidence: numberValue(object.confidence),
    provenance: stringValue(object.provenance),
    branch_id: stringValue(object.branch_id),
    raw: value
  };
}

function layoutNodes(records: GraphNodeRecord[]): ExplorerNode[] {
  return records.map((record, index) => {
    const column = index % 4;
    const row = Math.floor(index / 4);
    return {
      id: record.id,
      data: record,
      position: { x: column * 220, y: row * 130 },
      type: "default",
      style: {
        background: "rgba(24, 24, 27, 0.92)",
        border: "1px solid rgba(125, 211, 252, 0.35)",
        borderRadius: 8,
        boxShadow: "0 12px 28px rgba(0, 0, 0, 0.35)",
        color: "#e4e4e7",
        padding: 10,
        width: 180
      }
    };
  });
}

function layoutEdges(records: GraphEdgeRecord[]): ExplorerEdge[] {
  return records.map((record) => ({
    id: record.id,
    source: record.from_node_id,
    target: record.to_node_id,
    label: record.edge_type,
    data: record,
    markerEnd: { type: MarkerType.ArrowClosed },
    style: { stroke: "#7dd3fc" }
  }));
}

function firstMessage(result?: ApiResult<JsonValue>) {
  if (!result?.ok) {
    return undefined;
  }
  const data = objectValue(result.data);
  return stringValue(data?.message);
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
