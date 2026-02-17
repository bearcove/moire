import type {
  ConnectionsResponse,
  CutStatusResponse,
  SqlResponse,
  TriggerCutResponse,
} from "./types";

export type ApiMode = "live" | "lab";

export interface ApiClient {
  fetchConnections(): Promise<ConnectionsResponse>;
  triggerCut(): Promise<TriggerCutResponse>;
  fetchCutStatus(cutId: string): Promise<CutStatusResponse>;
  runSql(sql: string): Promise<SqlResponse>;
}
