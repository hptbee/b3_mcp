export const DEFAULT_API_BASE_URL = "http://127.0.0.1:7777";

export const API_BASE_URL =
  process.env.NEXT_PUBLIC_B3_API_BASE_URL?.replace(/\/$/, "") ??
  DEFAULT_API_BASE_URL;

export type JsonValue =
  | string
  | number
  | boolean
  | null
  | JsonValue[]
  | { [key: string]: JsonValue };

export type ApiResult<T> =
  | { ok: true; data: T; status: number }
  | { ok: false; error: string; status?: number; data?: JsonValue };

export type HealthResponse = {
  status: string;
  offline_mode: boolean;
  telemetry_enabled: boolean;
};

export type StatusResponse = {
  status: string;
  project_path: string;
  database_path: string;
  offline_mode: boolean;
  indexed_file_count: number;
  symbol_count: number;
  edge_count: number;
  current_branch?: string | null;
  mcp_runtime?: JsonValue;
};

export type ProjectResponse = {
  path: string;
  database_path: string;
  indexed_file_count: number;
  symbol_count: number;
  edge_count: number;
  offline_mode: boolean;
};

export type IndexSummary = {
  project_path: string;
  database_path: string;
  files_discovered: number;
  files_indexed: number;
  files_skipped: number;
  symbols_indexed: number;
  edges_indexed: number;
  parse_failures: number;
  duration_ms: number;
  reindex: boolean;
  behavior: string;
};

export type IndexStatusResponse = {
  status: string;
  started_at?: number | null;
  completed_at?: number | null;
  duration_ms?: number | null;
  files_discovered: number;
  files_indexed: number;
  files_skipped: number;
  symbols_indexed: number;
  edges_indexed: number;
  parse_failures: number;
  last_error?: string | null;
};

export type SavingsSummary = {
  records: number;
  estimated_tokens_saved: number;
  returned_tokens: number;
  avoided_file_reads: number;
  avoided_search_calls: number;
  partial: boolean;
};

export type QueryScope = {
  project_id: string;
  branch_id?: string;
};

export type QueryPayload = {
  query?: string;
  symbol?: string;
  scope: QueryScope;
  include_trace: boolean;
  limit: number;
  token_budget?: number;
};

export async function getJson<T>(path: string): Promise<ApiResult<T>> {
  return requestJson<T>(path, { method: "GET" });
}

export async function postJson<T>(
  path: string,
  body: unknown
): Promise<ApiResult<T>> {
  return requestJson<T>(path, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body)
  });
}

async function requestJson<T>(
  path: string,
  init: RequestInit
): Promise<ApiResult<T>> {
  try {
    const response = await fetch(`${API_BASE_URL}${path}`, {
      ...init,
      cache: "no-store"
    });
    const data = (await response.json().catch(() => null)) as JsonValue;

    if (!response.ok) {
      return {
        ok: false,
        status: response.status,
        error: errorMessage(data, response.statusText),
        data
      };
    }

    return { ok: true, status: response.status, data: data as T };
  } catch (error) {
    return {
      ok: false,
      error: error instanceof Error ? error.message : "request failed"
    };
  }
}

function errorMessage(data: JsonValue, fallback: string): string {
  if (
    data &&
    typeof data === "object" &&
    !Array.isArray(data) &&
    "error" in data
  ) {
    const error = data.error;
    if (
      error &&
      typeof error === "object" &&
      !Array.isArray(error) &&
      "message" in error &&
      typeof error.message === "string"
    ) {
      return error.message;
    }
  }

  return fallback || "request failed";
}
