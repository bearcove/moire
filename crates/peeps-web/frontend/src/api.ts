import type {
  JumpNowResponse,
  SqlRequest,
  SqlResponse,
  StuckRequest,
  SnapshotGraph,
  SnapshotNode,
  SnapshotEdge,
  UnresolvedEdge,
} from "./types";

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
    } catch {
      /* use status text */
    }
    throw new Error(msg);
  }
  return res.json() as Promise<T>;
}

export async function jumpNow(): Promise<JumpNowResponse> {
  return post<JumpNowResponse>("/api/jump-now", {});
}

export async function querySql(
  snapshotId: number,
  sql: string,
  params: (string | number | null)[] = [],
): Promise<SqlResponse> {
  const req: SqlRequest = { snapshot_id: snapshotId, sql, params };
  return post<SqlResponse>("/api/sql", req);
}

const STUCK_REQUEST_SQL = `SELECT
  r.id,
  json_extract(r.attrs_json, '$."request.method"') AS method,
  r.process,
  CAST(json_extract(r.attrs_json, '$.elapsed_ns') AS INTEGER) AS elapsed_ns,
  json_extract(r.attrs_json, '$."rpc.connection"') AS connection
FROM nodes r
WHERE r.kind = 'request'
  AND CAST(json_extract(r.attrs_json, '$.elapsed_ns') AS INTEGER) >= ?1
ORDER BY CAST(json_extract(r.attrs_json, '$.elapsed_ns') AS INTEGER) DESC
LIMIT 10;`;

export async function fetchStuckRequests(
  snapshotId: number,
  minElapsedNs: number,
): Promise<StuckRequest[]> {
  const resp = await querySql(snapshotId, STUCK_REQUEST_SQL, [minElapsedNs]);
  return resp.rows.map((row) => ({
    id: row[0] as string,
    method: row[1] as string | null,
    process: row[2] as string,
    elapsed_ns: row[3] as number,
    connection: row[4] as string | null,
  }));
}

const NODES_SQL = `SELECT id, kind, process, proc_key, attrs_json FROM nodes ORDER BY id`;
const EDGES_SQL = `SELECT src_id, dst_id, kind, attrs_json FROM edges ORDER BY src_id, dst_id`;
const UNRESOLVED_EDGES_SQL = `SELECT src_id, dst_id, missing_side, reason, referenced_proc_key, attrs_json FROM unresolved_edges ORDER BY src_id, dst_id`;

export async function fetchGraph(snapshotId: number): Promise<SnapshotGraph> {
  const [nodesResp, edgesResp, unresolvedResp] = await Promise.all([
    querySql(snapshotId, NODES_SQL),
    querySql(snapshotId, EDGES_SQL),
    querySql(snapshotId, UNRESOLVED_EDGES_SQL),
  ]);

  const nodes: SnapshotNode[] = nodesResp.rows.map((row) => ({
    id: row[0] as string,
    kind: row[1] as string,
    process: row[2] as string,
    proc_key: row[3] as string,
    attrs: JSON.parse((row[4] as string) || "{}"),
  }));

  const edges: SnapshotEdge[] = edgesResp.rows.map((row) => ({
    src_id: row[0] as string,
    dst_id: row[1] as string,
    kind: row[2] as string,
    attrs: JSON.parse((row[3] as string) || "{}"),
  }));

  const unresolvedEdges: UnresolvedEdge[] = unresolvedResp.rows.map((row) => ({
    src_id: row[0] as string,
    dst_id: row[1] as string,
    missing_side: row[2] as string,
    reason: row[3] as string,
    referenced_proc_key: row[4] as string | null,
    attrs: JSON.parse((row[5] as string) || "{}"),
  }));

  // Synthesize ghost nodes for unresolved edge endpoints
  const nodeIds = new Set(nodes.map((n) => n.id));
  const ghostMap = new Map<string, SnapshotNode>();

  for (const ue of unresolvedEdges) {
    // Determine which side(s) are missing and create ghost nodes
    const missingSrc = !nodeIds.has(ue.src_id) && !ghostMap.has(ue.src_id);
    const missingDst = !nodeIds.has(ue.dst_id) && !ghostMap.has(ue.dst_id);

    if (missingSrc) {
      ghostMap.set(ue.src_id, {
        id: ue.src_id,
        kind: "ghost",
        process: "",
        proc_key: ue.referenced_proc_key ?? "",
        attrs: {
          reason: ue.missing_side === "src" ? ue.reason : "missing_src",
          referenced_proc_key: ue.referenced_proc_key,
        },
      });
    }
    if (missingDst) {
      ghostMap.set(ue.dst_id, {
        id: ue.dst_id,
        kind: "ghost",
        process: "",
        proc_key: ue.referenced_proc_key ?? "",
        attrs: {
          reason: ue.missing_side === "dst" ? ue.reason : "missing_dst",
          referenced_proc_key: ue.referenced_proc_key,
        },
      });
    }

    // Add the unresolved edge as a regular edge so it renders in the graph
    edges.push({
      src_id: ue.src_id,
      dst_id: ue.dst_id,
      kind: "needs",
      attrs: ue.attrs,
    });
  }

  const ghostNodes = Array.from(ghostMap.values());

  return { nodes: [...nodes, ...ghostNodes], edges, ghostNodes };
}
