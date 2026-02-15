export interface JumpNowResponse {
  snapshot_id: number;
  requested: number;
  responded: number;
  timed_out: number;
}

export interface SqlRequest {
  snapshot_id: number;
  sql: string;
  params: (string | number | null)[];
}

export interface SqlResponse {
  snapshot_id: number;
  columns: string[];
  rows: (string | number | null)[][];
  row_count: number;
  truncated: boolean;
}

export interface StuckRequest {
  id: string;
  method: string | null;
  process: string;
  elapsed_ns: number;
  task_id: string | null;
  correlation_key: string | null;
}

export interface GraphNode {
  id: string;
  kind: string;
  label: string;
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface GraphEdge {
  source: string;
  target: string;
  kind: string;
}

export interface ElkInputNode {
  id: string;
  kind: string;
  label: string;
  width: number;
  height: number;
}

export interface ElkInputEdge {
  id: string;
  source: string;
  target: string;
  kind: string;
}

export interface ElkLayoutRequest {
  nodes: ElkInputNode[];
  edges: ElkInputEdge[];
}

export interface ElkLayoutResult {
  nodes: GraphNode[];
  edges: GraphEdge[];
  width: number;
  height: number;
}
