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

/** Filter out nodes of hidden kinds, bridging edges through them as pass-throughs. */
function filterHiddenKinds(graph: SnapshotGraph, hiddenKinds: Set<string>): SnapshotGraph {
  if (hiddenKinds.size === 0) return graph;

  const hiddenIds = new Set<string>();
  for (const n of graph.nodes) {
    if (hiddenKinds.has(n.kind)) hiddenIds.add(n.id);
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

  const handleSelectRequest = useCallback(
    (req: StuckRequest) => {
      const node = graph?.nodes.find((n) => n.id === req.id) ?? null;
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
    [graph],
  );

  const handleSelectGraphNode = useCallback(
    (nodeId: string) => {
      setSelectedNodeId(nodeId);
      setSelectedRequest(null);
      setSelectedEdge(null);
      const node = graph?.nodes.find((n) => n.id === nodeId) ?? null;
      setSelectedNode(node);
    },
    [graph],
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
    if (!graph) return [];
    const kinds = new Set<string>();
    for (const n of graph.nodes) {
      if (n.kind !== "ghost") kinds.add(n.kind);
    }
    return Array.from(kinds).sort();
  }, [graph]);

  const toggleKind = useCallback((kind: string) => {
    setHiddenKinds((prev) => {
      const next = new Set(prev);
      if (next.has(kind)) next.delete(kind);
      else next.add(kind);
      return next;
    });
  }, []);

  // Compute the displayed graph: full graph normally,
  // connected subgraph only when filtering via stuck request click.
  // Then apply node-kind hiding with pass-through edges.
  const displayGraph = useMemo(() => {
    if (!graph) return null;
    let g: SnapshotGraph = graph;
    if (filteredNodeId && graph.nodes.some((n) => n.id === filteredNodeId)) {
      g = connectedSubgraph(g, filteredNodeId);
    }
    return filterHiddenKinds(g, hiddenKinds);
  }, [graph, filteredNodeId, hiddenKinds]);

  const searchResults = useMemo(() => {
    if (!graph) return [];
    return searchGraphNodes(graph, graphSearchQuery).slice(0, 100);
  }, [graph, graphSearchQuery]);

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
          fullGraph={graph}
          filteredNodeId={filteredNodeId}
          selectedNodeId={selectedNodeId}
          selectedEdge={selectedEdge}
          searchQuery={graphSearchQuery}
          searchResults={searchResults}
          allKinds={allKinds}
          hiddenKinds={hiddenKinds}
          onToggleKind={toggleKind}
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
          graph={graph}
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
