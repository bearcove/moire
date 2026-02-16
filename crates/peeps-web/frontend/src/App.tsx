import { useCallback, useEffect, useMemo, useState } from "react";
import { WarningCircle } from "@phosphor-icons/react";
import { jumpNow, fetchStuckRequests, fetchGraph } from "./api";
import { Header } from "./components/Header";
import { RequestsTable } from "./components/RequestsTable";
import { GraphView } from "./components/GraphView";
import { Inspector } from "./components/Inspector";
import type { JumpNowResponse, StuckRequest, SnapshotGraph, SnapshotNode, SnapshotEdge } from "./types";

function useSessionState(key: string, initial: boolean): [boolean, () => void] {
  const [value, setValue] = useState(() => {
    const stored = sessionStorage.getItem(key);
    return stored !== null ? stored === "true" : initial;
  });
  const toggle = useCallback(() => {
    setValue((v) => {
      sessionStorage.setItem(key, String(!v));
      return !v;
    });
  }, [key]);
  return [value, toggle];
}

const MIN_ELAPSED_NS = 5_000_000_000; // 5 seconds

function firstNumAttr(attrs: Record<string, unknown>, keys: string[]): number | undefined {
  for (const k of keys) {
    const v = attrs[k];
    if (v == null || v === "") continue;
    const n = Number(v);
    if (!Number.isNaN(n)) return n;
  }
  return undefined;
}

/** BFS from a seed node, collecting all reachable nodes (both directions). */
function connectedSubgraph(graph: SnapshotGraph, seedId: string): SnapshotGraph {
  const adj = new Map<string, Set<string>>();
  for (const e of graph.edges) {
    let s = adj.get(e.src_id);
    if (!s) { s = new Set(); adj.set(e.src_id, s); }
    s.add(e.dst_id);
    let d = adj.get(e.dst_id);
    if (!d) { d = new Set(); adj.set(e.dst_id, d); }
    d.add(e.src_id);
  }

  const visited = new Set<string>();
  const queue = [seedId];
  while (queue.length > 0) {
    const id = queue.pop()!;
    if (visited.has(id)) continue;
    visited.add(id);
    const neighbors = adj.get(id);
    if (neighbors) {
      for (const n of neighbors) {
        if (!visited.has(n)) queue.push(n);
      }
    }
  }

  return {
    nodes: graph.nodes.filter((n) => visited.has(n.id)),
    edges: graph.edges.filter((e) => visited.has(e.src_id) && visited.has(e.dst_id)),
    ghostNodes: graph.ghostNodes.filter((n) => visited.has(n.id)),
  };
}

/** Filter out nodes matching a predicate, bridging edges through them as pass-throughs. */
function filterHiddenNodes(graph: SnapshotGraph, isHidden: (node: SnapshotNode) => boolean): SnapshotGraph {
  const hiddenIds = new Set<string>();
  for (const n of graph.nodes) {
    if (isHidden(n)) hiddenIds.add(n.id);
  }
  if (hiddenIds.size === 0) return graph;

  // Build forward adjacency from edges
  const fwd = new Map<string, Array<{ dst: string; edge: SnapshotEdge }>>();
  for (const e of graph.edges) {
    let list = fwd.get(e.src_id);
    if (!list) { list = []; fwd.set(e.src_id, list); }
    list.push({ dst: e.dst_id, edge: e });
  }

  // Edge kind priority for bridging: needs > spawned > touches
  function strongerKind(a: string, b: string): string {
    if (a === "needs" || b === "needs") return "needs";
    if (a === "spawned" || b === "spawned") return "spawned";
    return "touches";
  }

  // From a hidden node, BFS through hidden nodes to find all reachable visible destinations.
  // Returns array of { dst, kind } where kind is the strongest along the path.
  function reachableVisible(startId: string, initialKind: string): Array<{ dst: string; kind: string }> {
    const result: Array<{ dst: string; kind: string }> = [];
    const visited = new Set<string>();
    const queue: Array<{ id: string; kind: string }> = [{ id: startId, kind: initialKind }];

    while (queue.length > 0) {
      const { id, kind } = queue.pop()!;
      if (visited.has(id)) continue;
      visited.add(id);

      const outgoing = fwd.get(id);
      if (!outgoing) continue;
      for (const { dst, edge } of outgoing) {
        const combinedKind = strongerKind(kind, edge.kind);
        if (hiddenIds.has(dst)) {
          if (!visited.has(dst)) queue.push({ id: dst, kind: combinedKind });
        } else {
          result.push({ dst, kind: combinedKind });
        }
      }
    }
    return result;
  }

  // Build new edge list: keep direct visible→visible edges, bridge through hidden nodes
  const newEdges: SnapshotEdge[] = [];
  const seenBridges = new Set<string>();

  for (const e of graph.edges) {
    const srcHidden = hiddenIds.has(e.src_id);
    const dstHidden = hiddenIds.has(e.dst_id);

    if (!srcHidden && !dstHidden) {
      // Both visible: keep as-is
      newEdges.push(e);
    } else if (!srcHidden && dstHidden) {
      // Source visible, dest hidden: bridge through hidden chain
      for (const { dst, kind } of reachableVisible(e.dst_id, e.kind)) {
        const key = `${e.src_id}->${dst}:${kind}`;
        if (!seenBridges.has(key)) {
          seenBridges.add(key);
          newEdges.push({ src_id: e.src_id, dst_id: dst, kind, attrs: {} });
        }
      }
    }
    // srcHidden edges are handled when we encounter their visible predecessors
  }

  return {
    nodes: graph.nodes.filter((n) => !hiddenIds.has(n.id)),
    edges: newEdges,
    ghostNodes: graph.ghostNodes.filter((n) => !hiddenIds.has(n.id)),
  };
}

function searchGraphNodes(graph: SnapshotGraph, needle: string): SnapshotNode[] {
  const q = needle.trim().toLowerCase();
  if (!q) return [];
  return graph.nodes.filter((n) => JSON.stringify(n).toLowerCase().includes(q));
}

function enrichGraph(graph: SnapshotGraph): SnapshotGraph {
  const nodeIds = new Set(graph.nodes.map((n) => n.id));
  const needsEdges = graph.edges.filter((e) => e.kind === "needs");

  const outgoingNeeds = new Map<string, string[]>();
  const incomingNeeds = new Map<string, string[]>();
  for (const id of nodeIds) {
    outgoingNeeds.set(id, []);
    incomingNeeds.set(id, []);
  }
  for (const e of needsEdges) {
    if (!nodeIds.has(e.src_id) || !nodeIds.has(e.dst_id)) continue;
    outgoingNeeds.get(e.src_id)!.push(e.dst_id);
    incomingNeeds.get(e.dst_id)!.push(e.src_id);
  }

  // Tarjan SCC over directed `needs` edges to surface probable deadlock cycles.
  const indexById = new Map<string, number>();
  const lowlinkById = new Map<string, number>();
  const onStack = new Set<string>();
  const stack: string[] = [];
  const sccs: string[][] = [];
  let index = 0;

  function strongConnect(id: string) {
    indexById.set(id, index);
    lowlinkById.set(id, index);
    index += 1;
    stack.push(id);
    onStack.add(id);

    for (const dst of outgoingNeeds.get(id) ?? []) {
      if (!indexById.has(dst)) {
        strongConnect(dst);
        lowlinkById.set(id, Math.min(lowlinkById.get(id)!, lowlinkById.get(dst)!));
      } else if (onStack.has(dst)) {
        lowlinkById.set(id, Math.min(lowlinkById.get(id)!, indexById.get(dst)!));
      }
    }

    if (lowlinkById.get(id) === indexById.get(id)) {
      const component: string[] = [];
      while (stack.length > 0) {
        const w = stack.pop()!;
        onStack.delete(w);
        component.push(w);
        if (w === id) break;
      }
      sccs.push(component);
    }
  }

  for (const id of nodeIds) {
    if (!indexById.has(id)) strongConnect(id);
  }

  const cycleMetaById = new Map<string, { cycleId: string; cycleSize: number }>();
  let cycleOrdinal = 1;
  for (const scc of sccs) {
    const isSelfLoop =
      scc.length === 1 &&
      (outgoingNeeds.get(scc[0]) ?? []).some((dst) => dst === scc[0]);
    if (scc.length <= 1 && !isSelfLoop) continue;
    const cycleId = `cycle-${cycleOrdinal++}`;
    for (const id of scc) {
      cycleMetaById.set(id, { cycleId, cycleSize: scc.length });
    }
  }

  const enrichedNodes = graph.nodes.map((n) => {
    const blockers = outgoingNeeds.get(n.id) ?? [];
    const dependents = incomingNeeds.get(n.id) ?? [];
    const cycle = cycleMetaById.get(n.id);
    const attrs: Record<string, unknown> = {
      ...n.attrs,
      _ui_wait_blockers: blockers,
      _ui_wait_dependents: dependents,
      _ui_wait_blocker_count: blockers.length,
      _ui_wait_dependent_count: dependents.length,
    };

    if (cycle) {
      attrs._ui_cycle_id = cycle.cycleId;
      attrs._ui_cycle_size = cycle.cycleSize;
    }

    let deadlockReason: string | undefined;
    let deadlockAgeNs: number | undefined;

    if (n.kind === "future") {
      const pollInFlightNs = firstNumAttr(attrs, [
        "poll_in_flight_ns",
        "in_poll_ns",
        "current_poll_ns",
      ]);
      const idleNs = firstNumAttr(attrs, ["idle_ns", "last_polled_ns"]);
      if (pollInFlightNs != null && pollInFlightNs >= MIN_ELAPSED_NS) {
        deadlockReason = "in_poll_stuck";
        deadlockAgeNs = pollInFlightNs;
      } else if (idleNs != null && blockers.length > 0 && idleNs >= MIN_ELAPSED_NS) {
        deadlockReason = "pending_idle";
        deadlockAgeNs = idleNs;
      }
    } else {
      const waiters = firstNumAttr(attrs, [
        "waiters",
        "waiter_count",
        "send_waiters",
        "recv_waiters",
        "writer_waiters",
        "reader_waiters",
      ]);
      const oldestWaitNs = firstNumAttr(attrs, ["oldest_wait_ns", "longest_wait_ns"]);
      if ((waiters ?? 0) > 0 && blockers.length > 0) {
        deadlockReason = "contended_wait";
        deadlockAgeNs = oldestWaitNs;
      }
    }

    if (!deadlockReason && cycle) {
      deadlockReason = "needs_cycle";
    }

    if (deadlockReason) {
      attrs._ui_deadlock_candidate = true;
      attrs._ui_deadlock_reason = deadlockReason;
      if (deadlockAgeNs != null) attrs._ui_deadlock_age_ns = deadlockAgeNs;
    }

    return { ...n, attrs };
  });

  const cycleIdByNode = new Map<string, string>();
  for (const n of enrichedNodes) {
    const cycleId = n.attrs._ui_cycle_id;
    if (typeof cycleId === "string") cycleIdByNode.set(n.id, cycleId);
  }

  const enrichedEdges = graph.edges.map((e) => {
    const attrs = { ...e.attrs };
    if (
      e.kind === "needs" &&
      cycleIdByNode.get(e.src_id) &&
      cycleIdByNode.get(e.src_id) === cycleIdByNode.get(e.dst_id)
    ) {
      attrs._ui_cycle_edge = true;
    }
    return { ...e, attrs };
  });

  const enrichedById = new Map(enrichedNodes.map((n) => [n.id, n]));
  return {
    nodes: enrichedNodes,
    edges: enrichedEdges,
    ghostNodes: graph.ghostNodes.map((n) => enrichedById.get(n.id) ?? n),
  };
}

export function App() {
  const [snapshot, setSnapshot] = useState<JumpNowResponse | null>(null);
  const [requests, setRequests] = useState<StuckRequest[]>([]);
  const [graph, setGraph] = useState<SnapshotGraph | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const [selectedRequest, setSelectedRequest] = useState<StuckRequest | null>(null);
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
  const [filteredNodeId, setFilteredNodeId] = useState<string | null>(null);
  const [graphSearchQuery, setGraphSearchQuery] = useState("");
  const [selectedNode, setSelectedNode] = useState<SnapshotNode | null>(null);
  const [selectedEdge, setSelectedEdge] = useState<SnapshotEdge | null>(null);
  const [hiddenKinds, setHiddenKinds] = useState<Set<string>>(new Set());
  const [hiddenProcesses, setHiddenProcesses] = useState<Set<string>>(new Set());

  // Keep graph/inspector focus-first: left and right panels are collapsed by default,
  // but users can expand them and the state is sticky for the current browser session.
  const [leftCollapsed, toggleLeft] = useSessionState("peeps-left-collapsed", true);
  const [rightCollapsed, toggleRight] = useSessionState("peeps-right-collapsed", true);

  const handleJumpNow = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const snap = await jumpNow();
      setSnapshot(snap);
      const [stuck, graphData] = await Promise.all([
        fetchStuckRequests(snap.snapshot_id, MIN_ELAPSED_NS),
        fetchGraph(snap.snapshot_id),
      ]);
      setRequests(stuck);
      setGraph(graphData);
      setSelectedRequest(null);
      setSelectedNode(null);
      setSelectedNodeId(null);
      setSelectedEdge(null);
      setFilteredNodeId(null);
      setGraphSearchQuery("");
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    handleJumpNow();
  }, [handleJumpNow]);

  const enrichedGraph = useMemo(() => {
    if (!graph) return null;
    return enrichGraph(graph);
  }, [graph]);

  const handleSelectRequest = useCallback(
    (req: StuckRequest) => {
      const node = enrichedGraph?.nodes.find((n) => n.id === req.id) ?? null;
      setSelectedNodeId(req.id);
      setFilteredNodeId(req.id);
      setSelectedEdge(null);
      // Prefer full graph node metadata in inspector when available.
      if (node) {
        setSelectedNode(node);
        setSelectedRequest(null);
      } else {
        setSelectedNode(null);
        setSelectedRequest(req);
      }
    },
    [enrichedGraph],
  );

  const handleSelectGraphNode = useCallback(
    (nodeId: string) => {
      setSelectedNodeId(nodeId);
      setSelectedRequest(null);
      setSelectedEdge(null);
      const node = enrichedGraph?.nodes.find((n) => n.id === nodeId) ?? null;
      setSelectedNode(node);
    },
    [enrichedGraph],
  );

  const handleSelectEdge = useCallback(
    (edge: SnapshotEdge) => {
      setSelectedEdge(edge);
      setSelectedRequest(null);
      setSelectedNode(null);
      setSelectedNodeId(null);
    },
    [],
  );

  const handleClearSelection = useCallback(() => {
    setSelectedRequest(null);
    setSelectedNode(null);
    setSelectedNodeId(null);
    setSelectedEdge(null);
    setFilteredNodeId(null);
  }, []);

  // Collect all unique node kinds present in the graph (excluding ghosts).
  const allKinds = useMemo(() => {
    if (!enrichedGraph) return [];
    const kinds = new Set<string>();
    for (const n of enrichedGraph.nodes) {
      if (n.kind !== "ghost") kinds.add(n.kind);
    }
    return Array.from(kinds).sort();
  }, [enrichedGraph]);

  const allProcesses = useMemo(() => {
    if (!enrichedGraph) return [];
    const procs = new Set<string>();
    for (const n of enrichedGraph.nodes) {
      if (n.kind !== "ghost") procs.add(n.process);
    }
    return Array.from(procs).sort();
  }, [enrichedGraph]);

  const toggleKind = useCallback((kind: string) => {
    setHiddenKinds((prev) => {
      const next = new Set(prev);
      if (next.has(kind)) next.delete(kind);
      else next.add(kind);
      return next;
    });
  }, []);

  const toggleProcess = useCallback((process: string) => {
    setHiddenProcesses((prev) => {
      const next = new Set(prev);
      if (next.has(process)) next.delete(process);
      else next.add(process);
      return next;
    });
  }, []);

  const soloKind = useCallback((kind: string) => {
    setHiddenKinds((prev) => {
      // If this is already the only visible kind, show all
      const othersAllHidden = allKinds.every((k) => k === kind || prev.has(k));
      if (othersAllHidden && !prev.has(kind)) {
        return new Set();
      }
      // Otherwise, hide everything except this kind
      return new Set(allKinds.filter((k) => k !== kind));
    });
  }, [allKinds]);

  const soloProcess = useCallback((process: string) => {
    setHiddenProcesses((prev) => {
      // If this is already the only visible process, show all
      const othersAllHidden = allProcesses.every((p) => p === process || prev.has(p));
      if (othersAllHidden && !prev.has(process)) {
        return new Set();
      }
      // Otherwise, hide everything except this process
      return new Set(allProcesses.filter((p) => p !== process));
    });
  }, [allProcesses]);

  const hasActiveFilters = hiddenKinds.size > 0 || hiddenProcesses.size > 0 || filteredNodeId != null || graphSearchQuery.trim().length > 0;

  const handleResetFilters = useCallback(() => {
    setHiddenKinds(new Set());
    setHiddenProcesses(new Set());
    setFilteredNodeId(null);
    setGraphSearchQuery("");
  }, []);

  // Compute the displayed graph: full graph normally,
  // connected subgraph only when filtering via stuck request click.
  // Then apply node-kind hiding with pass-through edges.
  const displayGraph = useMemo(() => {
    if (!enrichedGraph) return null;
    let g: SnapshotGraph = enrichedGraph;
    if (filteredNodeId && enrichedGraph.nodes.some((n) => n.id === filteredNodeId)) {
      g = connectedSubgraph(g, filteredNodeId);
    }
    g = filterHiddenNodes(g, (n) => hiddenKinds.has(n.kind));
    g = filterHiddenNodes(g, (n) => hiddenProcesses.has(n.process));
    return g;
  }, [enrichedGraph, filteredNodeId, hiddenKinds, hiddenProcesses]);

  const searchResults = useMemo(() => {
    if (!enrichedGraph) return [];
    return searchGraphNodes(enrichedGraph, graphSearchQuery).slice(0, 100);
  }, [enrichedGraph, graphSearchQuery]);

  const handleSelectSearchResult = useCallback(
    (nodeId: string) => {
      setFilteredNodeId(null);
      handleSelectGraphNode(nodeId);
    },
    [handleSelectGraphNode],
  );

  return (
    <div className="app">
      <Header snapshot={snapshot} loading={loading} onJumpNow={handleJumpNow} />
      {error && (
        <div className="status-bar">
          <WarningCircle
            size={14}
            weight="bold"
            style={{ color: "light-dark(#d30000, #ff6b6b)", flexShrink: 0 }}
          />
          <span className="error-text">{error}</span>
        </div>
      )}
      <div
        className={[
          "main-content",
          leftCollapsed && "main-content--left-collapsed",
          rightCollapsed && "main-content--right-collapsed",
        ].filter(Boolean).join(" ")}
      >
        <RequestsTable
          requests={requests}
          selectedId={selectedNodeId}
          onSelect={handleSelectRequest}
          collapsed={leftCollapsed}
          onToggleCollapse={toggleLeft}
        />
        <GraphView
          graph={displayGraph}
          fullGraph={enrichedGraph}
          filteredNodeId={filteredNodeId}
          selectedNodeId={selectedNodeId}
          selectedEdge={selectedEdge}
          searchQuery={graphSearchQuery}
          searchResults={searchResults}
          allKinds={allKinds}
          hiddenKinds={hiddenKinds}
          onToggleKind={toggleKind}
          onSoloKind={soloKind}
          allProcesses={allProcesses}
          hiddenProcesses={hiddenProcesses}
          onToggleProcess={toggleProcess}
          onSoloProcess={soloProcess}
          hasActiveFilters={hasActiveFilters}
          onResetFilters={handleResetFilters}
          onSearchQueryChange={setGraphSearchQuery}
          onSelectSearchResult={handleSelectSearchResult}
          onSelectNode={handleSelectGraphNode}
          onSelectEdge={handleSelectEdge}
          onClearSelection={handleClearSelection}
        />
        <Inspector
          selectedRequest={selectedRequest}
          selectedNode={selectedNode}
          selectedEdge={selectedEdge}
          graph={enrichedGraph}
          filteredNodeId={filteredNodeId}
          onFocusNode={setFilteredNodeId}
          onSelectNode={handleSelectGraphNode}
          collapsed={rightCollapsed}
          onToggleCollapse={toggleRight}
        />
      </div>
    </div>
  );
}
