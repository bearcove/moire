import type { JumpNowResponse, SqlRequest, SqlResponse, StuckRequest } from "./types";

async function post<T>(url: string, body: unknown): Promise<T> {
  const res = await fetch(url, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  if (!res.ok) {
    const text = await res.text();
    let msg = `${res.status} ${res.statusText}`;
    try {
      const err = JSON.parse(text);
      if (err.error) msg = err.error;
    } catch { /* use status text */ }
    throw new Error(msg);
  }
  return res.json() as Promise<T>;
}

export async function jumpNow(): Promise<JumpNowResponse> {
  return post<JumpNowResponse>("/api/jump-now", {});
}

export async function querySql(snapshotId: number, sql: string, params: (string | number | null)[] = []): Promise<SqlResponse> {
  const req: SqlRequest = { snapshot_id: snapshotId, sql, params };
  return post<SqlResponse>("/api/sql", req);
}

const STUCK_REQUEST_SQL = `SELECT
  r.id,
  json_extract(r.attrs_json, '$.method') AS method,
  r.process,
  CAST(json_extract(r.attrs_json, '$.elapsed_ns') AS INTEGER) AS elapsed_ns,
  json_extract(r.attrs_json, '$.task_id') AS task_id,
  json_extract(r.attrs_json, '$.correlation_key') AS correlation_key
FROM nodes r
LEFT JOIN nodes resp
  ON resp.kind = 'response'
 AND json_extract(resp.attrs_json, '$.correlation_key') = json_extract(r.attrs_json, '$.correlation_key')
WHERE r.kind = 'request'
  AND CAST(json_extract(r.attrs_json, '$.elapsed_ns') AS INTEGER) >= ?1
  AND (resp.id IS NULL OR json_extract(resp.attrs_json, '$.status') = 'in_flight')
ORDER BY CAST(json_extract(r.attrs_json, '$.elapsed_ns') AS INTEGER) DESC
LIMIT 500;`;

export async function fetchStuckRequests(snapshotId: number, minElapsedNs: number): Promise<StuckRequest[]> {
  const resp = await querySql(snapshotId, STUCK_REQUEST_SQL, [minElapsedNs]);
  return resp.rows.map((row) => ({
    id: row[0] as string,
    method: row[1] as string | null,
    process: row[2] as string,
    elapsed_ns: row[3] as number,
    task_id: row[4] as string | null,
    correlation_key: row[5] as string | null,
  }));
}
