import { useMemo, useState } from "react";
import { CaretDown, CaretLeft, CaretRight, Plugs } from "@phosphor-icons/react";
import { isResourceKind } from "../resourceKinds";
import type { SnapshotGraph } from "../types";

type SortKey = "health" | "connection" | "state" | "pending" | "last_recv" | "last_sent";
type SortDir = "asc" | "desc";
type Health = "healthy" | "warning" | "critical";
type SeverityFilter = "all" | "warning_plus" | "critical";

const WARN_PENDING = 10;
const CRIT_PENDING = 25;
const WARN_STALE_NS = 15_000_000_000;
const CRIT_STALE_NS = 60_000_000_000;

interface ResourcesPanelProps {
  graph: SnapshotGraph | null;
  snapshotCapturedAtNs: number | null;
  selectedNodeId: string | null;
  onSelectNode: (nodeId: string) => void;
  collapsed: boolean;
  onToggleCollapse: () => void;
}

interface ConnectionRow {
  nodeId: string;
  connectionId: string;
  state: string;
  pending: number | null;
  lastRecvAgeNs: number | null;
  lastSentAgeNs: number | null;
  health: Health;
}

function firstString(attrs: Record<string, unknown>, keys: string[]): string | undefined {
  for (const k of keys) {
    const v = attrs[k];
    if (v != null && v !== "") return String(v);
  }
  return undefined;
}

function firstNumber(attrs: Record<string, unknown>, keys: string[]): number | undefined {
  for (const k of keys) {
    const v = attrs[k];
    if (v == null || v === "") continue;
    const n = Number(v);
    if (!Number.isNaN(n)) return n;
  }
  return undefined;
}

function formatAge(ageNs: number | null): string {
  if (ageNs == null) return "—";
  if (ageNs < 1_000_000) return `${Math.round(ageNs / 1_000)}us ago`;
  if (ageNs < 1_000_000_000) return `${Math.round(ageNs / 1_000_000)}ms ago`;
  const seconds = ageNs / 1_000_000_000;
  if (seconds < 60) return `${seconds.toFixed(1)}s ago`;
  return `${(seconds / 60).toFixed(1)}m ago`;
}

function connectionToken(nodeId: string, attrs: Record<string, unknown>): string {
  return (
    firstString(attrs, ["connection.id", "rpc.connection", "connection"]) ??
    (nodeId.startsWith("connection:") ? nodeId.slice("connection:".length) : nodeId)
  );
}

function connectionState(attrs: Record<string, unknown>): string {
  const state = firstString(attrs, ["connection.state", "state"]);
  if (state === "open" || state === "closed") return state;
  return "unknown";
}

function toAgeNs(snapshotCapturedAtNs: number | null, tsNs: number | undefined): number | null {
  if (snapshotCapturedAtNs == null || tsNs == null) return null;
  if (!Number.isFinite(snapshotCapturedAtNs) || !Number.isFinite(tsNs)) return null;
  return Math.max(0, snapshotCapturedAtNs - tsNs);
}

function connectionHealth(pending: number | null, lastRecvAgeNs: number | null): Health {
  if ((pending ?? 0) >= CRIT_PENDING) return "critical";
  if ((pending ?? 0) >= WARN_PENDING) return "warning";
  if ((lastRecvAgeNs ?? -1) >= CRIT_STALE_NS) return "critical";
  if ((lastRecvAgeNs ?? -1) >= WARN_STALE_NS) return "warning";
  return "healthy";
}

function healthRank(health: Health): number {
  if (health === "critical") return 2;
  if (health === "warning") return 1;
  return 0;
}

function healthAtLeast(row: ConnectionRow, filter: SeverityFilter): boolean {
  if (filter === "all") return true;
  if (filter === "critical") return row.health === "critical";
  return row.health === "critical" || row.health === "warning";
}

function sortRows(rows: ConnectionRow[], key: SortKey, dir: SortDir): ConnectionRow[] {
  const sign = dir === "asc" ? 1 : -1;
  const stateRank = (state: string) => (state === "open" ? 2 : state === "closed" ? 1 : 0);

  const sorted = [...rows];
  sorted.sort((a, b) => {
    const cmpNumber = (av: number | null, bv: number | null, missingLast: boolean): number => {
      if (av == null && bv == null) return 0;
      if (av == null) return missingLast ? 1 : -1;
      if (bv == null) return missingLast ? -1 : 1;
      return av - bv;
    };

    let primary = 0;
    if (key === "health") primary = healthRank(a.health) - healthRank(b.health);
    if (key === "connection") primary = a.connectionId.localeCompare(b.connectionId);
    if (key === "state") primary = stateRank(a.state) - stateRank(b.state);
    if (key === "pending") primary = cmpNumber(a.pending, b.pending, true);
    if (key === "last_recv") primary = cmpNumber(a.lastRecvAgeNs, b.lastRecvAgeNs, true);
    if (key === "last_sent") primary = cmpNumber(a.lastSentAgeNs, b.lastSentAgeNs, true);

    if (primary !== 0) return primary * sign;

    // Default operator-first tie-break when sorting by pending:
    // highest pending first, then stalest recv first.
    if (key === "pending") {
      const byRecvAge = cmpNumber(a.lastRecvAgeNs, b.lastRecvAgeNs, true);
      if (byRecvAge !== 0) return byRecvAge * -1;
    }

    // Deterministic tie-break for stable rendering.
    if (a.nodeId !== b.nodeId) return a.nodeId.localeCompare(b.nodeId);
    return 0;
  });
  return sorted;
}

export function ResourcesPanel({
  graph,
  snapshotCapturedAtNs,
  selectedNodeId,
  onSelectNode,
  collapsed,
  onToggleCollapse,
}: ResourcesPanelProps) {
  const [sortKey, setSortKey] = useState<SortKey>("pending");
  const [sortDir, setSortDir] = useState<SortDir>("desc");
  const [severityFilter, setSeverityFilter] = useState<SeverityFilter>("all");

  const rows = useMemo(() => {
    if (!graph) return [] as ConnectionRow[];
    const connectionRows = graph.nodes
      .filter((node) => node.kind === "connection" && isResourceKind(node.kind))
      .map((node) => {
        const pending = firstNumber(node.attrs, ["connection.pending_requests", "pending_requests"]);
        const lastRecvTsNs = firstNumber(node.attrs, ["connection.last_frame_recv_at_ns", "last_frame_recv_at_ns"]);
        const lastSentTsNs = firstNumber(node.attrs, ["connection.last_frame_sent_at_ns", "last_frame_sent_at_ns"]);
        const lastRecvAgeNs = toAgeNs(snapshotCapturedAtNs, lastRecvTsNs);
        const pendingValue = pending ?? null;
        return {
          nodeId: node.id,
          connectionId: connectionToken(node.id, node.attrs),
          state: connectionState(node.attrs),
          pending: pendingValue,
          lastRecvAgeNs,
          lastSentAgeNs: toAgeNs(snapshotCapturedAtNs, lastSentTsNs),
          health: connectionHealth(pendingValue, lastRecvAgeNs),
        } satisfies ConnectionRow;
      });

    return sortRows(connectionRows, sortKey, sortDir);
  }, [graph, snapshotCapturedAtNs, sortDir, sortKey]);

  const visibleRows = useMemo(
    () => rows.filter((row) => healthAtLeast(row, severityFilter)),
    [rows, severityFilter],
  );

  const summary = useMemo(() => {
    const warningCount = rows.filter((row) => row.health === "warning").length;
    const criticalCount = rows.filter((row) => row.health === "critical").length;
    return { total: rows.length, warningCount, criticalCount };
  }, [rows]);

  function toggleSort(nextKey: SortKey) {
    if (sortKey === nextKey) {
      setSortDir((prev) => (prev === "asc" ? "desc" : "asc"));
      return;
    }
    setSortKey(nextKey);
    // Default operator-friendly direction per column.
    setSortDir(nextKey === "connection" || nextKey === "state" ? "asc" : "desc");
  }

  function sortArrow(key: SortKey): string {
    if (sortKey !== key) return "";
    return sortDir === "asc" ? " \u2191" : " \u2193";
  }

  if (collapsed) {
    return (
      <div className="panel panel--resources-collapsed">
        <button className="panel-collapse-btn" onClick={onToggleCollapse} title="Expand panel">
          <CaretRight size={14} weight="bold" />
        </button>
        <span className="resources-collapsed-label">Resources</span>
      </div>
    );
  }

  return (
    <div className="panel panel--resources">
      <div className="panel-header">
        <Plugs size={14} weight="bold" /> Resources ({summary.total})
        <button className="panel-collapse-btn" onClick={onToggleCollapse} title="Collapse panel">
          <CaretLeft size={14} weight="bold" />
        </button>
      </div>

      <div className="resources-summary-row">
        <span className="resources-chip">total {summary.total}</span>
        <span className="resources-chip resources-chip--warn">warning {summary.warningCount}</span>
        <span className="resources-chip resources-chip--crit">critical {summary.criticalCount}</span>
      </div>

      <div className="resources-filter-row">
        <button
          type="button"
          className={`resources-filter-btn${severityFilter === "all" ? " resources-filter-btn--active" : ""}`}
          onClick={() => setSeverityFilter("all")}
        >
          All
        </button>
        <button
          type="button"
          className={`resources-filter-btn${severityFilter === "warning_plus" ? " resources-filter-btn--active" : ""}`}
          onClick={() => setSeverityFilter("warning_plus")}
        >
          Warning+
        </button>
        <button
          type="button"
          className={`resources-filter-btn${severityFilter === "critical" ? " resources-filter-btn--active" : ""}`}
          onClick={() => setSeverityFilter("critical")}
        >
          Critical
        </button>
      </div>

      {rows.length === 0 ? (
        <div className="resources-empty">No connection resources in this snapshot.</div>
      ) : visibleRows.length === 0 ? (
        <div className="resources-empty">No connections match this health filter.</div>
      ) : (
        <div className="resources-table-wrap">
          <table className="resources-table">
            <thead>
              <tr>
                <th>
                  <button type="button" className="resources-sort" onClick={() => toggleSort("health")}>
                    Health{sortArrow("health")}
                  </button>
                </th>
                <th>
                  <button type="button" className="resources-sort" onClick={() => toggleSort("connection")}>
                    Connection{sortArrow("connection")}
                  </button>
                </th>
                <th>
                  <button type="button" className="resources-sort" onClick={() => toggleSort("state")}>
                    State{sortArrow("state")}
                  </button>
                </th>
                <th>
                  <button type="button" className="resources-sort" onClick={() => toggleSort("pending")}>
                    Pending{sortArrow("pending")}
                  </button>
                </th>
                <th>
                  <button type="button" className="resources-sort" onClick={() => toggleSort("last_recv")}>
                    Last recv{sortArrow("last_recv")}
                  </button>
                </th>
                <th>
                  <button type="button" className="resources-sort" onClick={() => toggleSort("last_sent")}>
                    Last sent{sortArrow("last_sent")}
                  </button>
                </th>
              </tr>
            </thead>
            <tbody>
              {visibleRows.map((row) => (
                <tr
                  key={row.nodeId}
                  className={selectedNodeId === row.nodeId ? "resources-row resources-row--selected" : "resources-row"}
                  onClick={() => onSelectNode(row.nodeId)}
                  title={row.nodeId}
                >
                  <td>
                    <span
                      className={`resources-health-pill resources-health-pill--${
                        row.health === "critical"
                          ? "crit"
                          : row.health === "warning"
                            ? "warn"
                            : "ok"
                      }`}
                    >
                      {row.health}
                    </span>
                  </td>
                  <td className="resources-cell-mono">{row.connectionId}</td>
                  <td>{row.state}</td>
                  <td>{row.pending != null ? row.pending : "—"}</td>
                  <td className="resources-cell-mono">{formatAge(row.lastRecvAgeNs)}</td>
                  <td className="resources-cell-mono">{formatAge(row.lastSentAgeNs)}</td>
                </tr>
              ))}
            </tbody>
          </table>
          <div className="resources-sort-hint">
            <CaretDown size={10} weight="bold" /> Click column headers to sort.
          </div>
        </div>
      )}
    </div>
  );
}
