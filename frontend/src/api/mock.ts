import type { ApiClient } from "./client";
import type {
  ConnectionsResponse,
  CutStatusResponse,
  SqlResponse,
  TriggerCutResponse,
} from "./types";

const sampleConnections: ConnectionsResponse = {
  connected_processes: 2,
  processes: [
    { conn_id: 101, process_name: "lab-server", pid: 4242 },
    { conn_id: 202, process_name: "lab-loader", pid: 1313 },
  ],
};

const sampleSqlPreview: SqlResponse = {
  columns: ["conn_id", "process_name", "pid", "connected_at_ns"],
  rows: sampleConnections.processes.map((proc) => [proc.conn_id, proc.process_name, proc.pid, Date.now()]),
  row_count: sampleConnections.processes.length,
};

const retryDelay = 120;

function delay<T>(payload: T, ms = retryDelay): Promise<T> {
  return new Promise((resolve) => {
    window.setTimeout(() => resolve(payload), ms);
  });
}

function buildPendingIds(count: number): number[] {
  return sampleConnections.processes.slice(0, count).map((proc) => proc.conn_id);
}

export function createMockApiClient(): ApiClient {
  let nextCutId = 1;
  let activeCut: CutStatusResponse | null = null;
  return {
    fetchConnections: () => delay(sampleConnections),
    triggerCut: () => {
      const cut: CutStatusResponse = {
        cut_id: `lab-mock-${String(nextCutId).padStart(3, "0")}`,
        requested_at_ns: Date.now() * 1_000_000,
        pending_connections: sampleConnections.processes.length,
        acked_connections: 0,
        pending_conn_ids: buildPendingIds(sampleConnections.processes.length),
      } as CutStatusResponse;
      nextCutId += 1;
      activeCut = cut;
      const trigger: TriggerCutResponse = {
        cut_id: cut.cut_id,
        requested_at_ns: cut.requested_at_ns,
        requested_connections: sampleConnections.processes.length,
      };
      return delay(trigger);
    },
    fetchCutStatus: (cutId: string) => {
      if (!activeCut || activeCut.cut_id !== cutId) {
        return delay({
          cut_id: cutId,
          requested_at_ns: Date.now() * 1_000_000,
          pending_connections: 0,
          acked_connections: 0,
          pending_conn_ids: [],
        });
      }
      const pending = Math.max(activeCut.pending_connections - 1, 0);
      const acked = activeCut.acked_connections + (activeCut.pending_connections > 0 ? 1 : 0);
      activeCut = {
        ...activeCut,
        pending_connections: pending,
        acked_connections: acked,
        pending_conn_ids: buildPendingIds(pending),
      };
      return delay(activeCut);
    },
    runSql: () => delay(sampleSqlPreview),
  };
}
